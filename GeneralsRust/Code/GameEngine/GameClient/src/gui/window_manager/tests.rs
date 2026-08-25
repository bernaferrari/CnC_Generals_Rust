//! WindowManager unit tests (parity with C++ GameWindowManager).

use super::*;
use super::{
    apply_window_widget_data, hide_window_rc, queue_create_layout, queue_set_focus,
    queue_window_manager_op, with_window_manager, with_window_manager_ref,
};
use crate::gui::gadgets::{TextAlignment, VerticalAlignment};
use crate::gui::game_window::*;
use crate::gui::window_script::{
    ComboBoxData, StaticTextData, TabControlData as ScriptTabControlData, WindowDefinition,
    WindowLayoutDefinition,
};
use crate::input::with_mouse;
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

static TEST_MOUSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock_test_mouse() -> MutexGuard<'static, ()> {
    TEST_MOUSE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[test]
fn test_window_manager_creation() {
    let manager = WindowManager::new();
    assert_eq!(manager.window_count, 0);
    assert!(manager.root_windows.is_empty());
    assert!(manager.get_focus().is_none());
}

#[test]
fn bind_window_callbacks_preserves_editor_callback_names_like_cpp() {
    let manager = WindowManager::new();
    let mut window = GameWindow::new();
    let window_def = WindowDefinition {
        system_callback: "GameWinDefaultSystem".to_string(),
        input_callback: "GameWinDefaultInput".to_string(),
        tooltip_callback: "GameWinDefaultTooltip".to_string(),
        draw_callback: "W3DNoDraw".to_string(),
        ..WindowDefinition::default()
    };

    window.set_edit_data(Some(GameWindowEditData::default()));
    manager.bind_window_callbacks(&mut window, &window_def);

    let edit_data = window.get_edit_data().unwrap();
    assert_eq!(edit_data.system_callback_string, "GameWinDefaultSystem");
    assert_eq!(edit_data.input_callback_string, "GameWinDefaultInput");
    assert_eq!(edit_data.tooltip_callback_string, "GameWinDefaultTooltip");
    assert_eq!(edit_data.draw_callback_string, "W3DNoDraw");
}

#[test]
fn bind_none_callback_does_not_overwrite_existing_system_like_cpp() {
    use std::cell::Cell;

    let manager = WindowManager::new();
    let mut window = GameWindow::new();
    let hit = Rc::new(Cell::new(false));
    {
        let hit = Rc::clone(&hit);
        window.set_system_callback(move |_, _, _, _| {
            hit.set(true);
            WindowMsgHandled::Handled
        });
    }
    window.set_edit_data(Some(GameWindowEditData::default()));
    let window_def = WindowDefinition {
        system_callback: "[None]".to_string(),
        input_callback: "[None]".to_string(),
        tooltip_callback: "[None]".to_string(),
        draw_callback: "[None]".to_string(),
        ..WindowDefinition::default()
    };
    manager.bind_window_callbacks(&mut window, &window_def);

    let edit_data = window.get_edit_data().unwrap();
    assert_eq!(edit_data.system_callback_string, "[None]");
    assert_eq!(edit_data.input_callback_string, "[None]");

    let _ = window.send_system_message(WindowMessage::GadgetSelected, 1, 0);
    assert!(
        hit.get(),
        "C++ createWindow skips winSetSystemFunc when SYSTEMCALLBACK=[None]"
    );
}

#[test]
fn static_text_script_data_applies_cpp_alignment_defaults() {
    let mut window = GameWindow::new();
    window.set_widget(WindowWidget::StaticText(StaticText::new(7, 0, 0, 120, 24)));
    let window_def = WindowDefinition {
        static_text_data: Some(StaticTextData {
            centered: false,
            centered_vertically: true,
            left_margin: 7,
            top_margin: 7,
        }),
        ..WindowDefinition::default()
    };

    apply_window_widget_data(&mut window, &window_def);

    let Some(WindowWidget::StaticText(label)) = window.widget() else {
        panic!("static text widget missing");
    };
    let cfg = label.config();
    assert_eq!(cfg.alignment, TextAlignment::Left);
    assert_eq!(cfg.vertical_alignment, VerticalAlignment::Center);
    assert_eq!(cfg.left_margin, 7);
    assert_eq!(cfg.top_margin, 7);
}

#[test]
fn test_window_creation() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    assert_eq!(manager.window_count, 1);
    assert_eq!(manager.root_windows.len(), 1);

    let window_id = window.borrow().get_id();
    assert!(window_id > 0);

    let found_window = manager.get_window_by_id(window_id).unwrap();
    assert!(Rc::ptr_eq(&window, &found_window));
}

#[test]
fn script_child_creation_sends_script_create_input_to_parent_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));

    parent.borrow_mut().set_status(WindowStatus::NO_INPUT);
    {
        let seen = Rc::clone(&seen);
        parent
            .borrow_mut()
            .set_input_callback(move |_, msg, data1, data2| {
                seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    let layout = Rc::new(RefCell::new(WindowLayout::new("test.wnd".to_string())));
    let layout_def = WindowLayoutDefinition::default();
    let mut info = WindowLayoutInfo::default();
    let child_name = "test.wnd:ChildFromScript";
    let child_id = NameKeyGenerator::name_to_key(child_name) as WindowId;
    let child_def = WindowDefinition {
        name: child_name.to_string(),
        position: (5, 6),
        size: (20, 30),
        ..WindowDefinition::default()
    };

    manager
        .create_window_from_definition(&child_def, Some(&parent), &layout, &layout_def, &mut info)
        .unwrap();

    assert_eq!(
        seen.borrow().as_slice(),
        &[(WindowMessage::ScriptCreate, child_id as WindowMsgData, 0)]
    );
}

#[test]
fn combo_edit_child_draws_from_start_only_when_non_editable_like_cpp() {
    for (is_editable, expected_draw_from_start) in [(false, true), (true, false)] {
        let mut manager = WindowManager::new();
        let layout = Rc::new(RefCell::new(WindowLayout::new("test.wnd".to_string())));
        let layout_def = WindowLayoutDefinition::default();
        let mut info = WindowLayoutInfo::default();
        let combo_def = WindowDefinition {
            name: format!("test.wnd:Combo{}", is_editable),
            window_type: "COMBOBOX".to_string(),
            status: WindowStatus::ENABLED,
            style: GWS_COMBO_BOX,
            size: (120, 20),
            combo_box_data: Some(ComboBoxData {
                is_editable,
                ..Default::default()
            }),
            ..WindowDefinition::default()
        };

        let combo = manager
            .create_window_from_definition(&combo_def, None, &layout, &layout_def, &mut info)
            .unwrap();
        let links = combo.borrow().combobox_links().unwrap();
        let edit = combo.borrow().find_child_by_id(links.edit_box).unwrap();
        let edit_borrow = edit.borrow();
        let WindowWidget::TextEntry(entry) = edit_borrow.widget().unwrap() else {
            panic!("combo edit child should be a text entry");
        };

        assert_eq!(entry.draw_text_from_start(), expected_draw_from_start);
        assert_eq!(
            edit_borrow.get_status().contains(WindowStatus::NO_INPUT),
            !is_editable
        );
    }
}

#[test]
fn script_tab_control_shows_first_script_pane_in_cpp_fixup_order() {
    let mut manager = WindowManager::new();
    let layout = Rc::new(RefCell::new(WindowLayout::new("test.wnd".to_string())));
    let layout_def = WindowLayoutDefinition::default();
    let mut info = WindowLayoutInfo::default();
    let first_pane_name = "test.wnd:FirstPane";
    let second_pane_name = "test.wnd:SecondPane";
    let tab_def = WindowDefinition {
        name: "test.wnd:Tabs".to_string(),
        window_type: "TABCONTROL".to_string(),
        style: GWS_TAB_CONTROL,
        position: (0, 0),
        size: (200, 100),
        tab_control_data: Some(ScriptTabControlData {
            tab_count: 2,
            ..Default::default()
        }),
        children: vec![
            WindowDefinition {
                name: first_pane_name.to_string(),
                window_type: "TABPANE".to_string(),
                style: GWS_TAB_PANE,
                position: (0, 0),
                size: (200, 80),
                ..WindowDefinition::default()
            },
            WindowDefinition {
                name: second_pane_name.to_string(),
                window_type: "TABPANE".to_string(),
                style: GWS_TAB_PANE,
                position: (0, 0),
                size: (200, 80),
                ..WindowDefinition::default()
            },
        ],
        ..WindowDefinition::default()
    };

    manager
        .create_window_from_definition(&tab_def, None, &layout, &layout_def, &mut info)
        .unwrap();

    let first = manager
        .get_window_by_id(NameKeyGenerator::name_to_key(first_pane_name) as WindowId)
        .unwrap();
    let second = manager
        .get_window_by_id(NameKeyGenerator::name_to_key(second_pane_name) as WindowId)
        .unwrap();

    assert!(!first.borrow().is_hidden());
    assert!(second.borrow().is_hidden());
}

#[test]
fn root_window_owner_defaults_to_self_like_cpp() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    let owner = window.borrow().get_owner().unwrap();

    assert!(Rc::ptr_eq(&window, &owner));
    assert!(window.borrow().owner_is_self());
}

#[test]
fn test_window_hierarchy() {
    let mut manager = WindowManager::new();

    let parent = manager.create_window(None, 0, 0, 200, 200).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 50, 50)
        .unwrap();

    assert_eq!(manager.window_count, 2);
    assert_eq!(manager.root_windows.len(), 1); // Only parent is root

    let parent_borrow = parent.borrow();
    assert!(parent_borrow.is_child(&*child.borrow()));

    let child_borrow = child.borrow();
    let child_parent = child_borrow.get_parent().unwrap();
    assert!(Rc::ptr_eq(&parent, &child_parent));
}

#[test]
fn child_window_owner_defaults_to_parent_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 200, 200).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 50, 50)
        .unwrap();

    let owner = child.borrow().get_owner().unwrap();

    assert!(Rc::ptr_eq(&parent, &owner));
    assert!(!child.borrow().owner_is_self());
}

#[test]
fn set_window_owner_none_defaults_to_self_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 200, 200).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 50, 50)
        .unwrap();

    manager.set_window_owner(&child, None).unwrap();

    let owner = child.borrow().get_owner().unwrap();
    assert!(Rc::ptr_eq(&child, &owner));
    assert!(child.borrow().owner_is_self());
}

#[test]
fn set_window_parent_moves_root_to_child_list_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 200, 200).unwrap();
    let window = manager.create_window(None, 10, 10, 50, 50).unwrap();

    manager.set_window_parent(&window, Some(&parent)).unwrap();

    assert_eq!(manager.root_windows.len(), 1);
    assert!(Rc::ptr_eq(&manager.root_windows[0], &parent));
    assert!(parent.borrow().is_child(&window.borrow()));
    assert!(Rc::ptr_eq(&window.borrow().get_parent().unwrap(), &parent));
}

#[test]
fn set_window_parent_moves_child_to_root_list_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 200, 200).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 50, 50)
        .unwrap();

    manager.set_window_parent(&child, None).unwrap();

    assert_eq!(manager.root_windows.len(), 2);
    assert!(Rc::ptr_eq(&manager.root_windows[0], &child));
    assert!(Rc::ptr_eq(&manager.root_windows[1], &parent));
    assert!(child.borrow().get_parent().is_none());
    assert!(!parent.borrow().is_child(&child.borrow()));
}

#[test]
fn new_child_windows_insert_at_head_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let first = manager
        .create_window(Some(&parent), 10, 10, 20, 20)
        .unwrap();
    let second = manager
        .create_window(Some(&parent), 20, 20, 20, 20)
        .unwrap();

    let parent = parent.borrow();
    assert!(Rc::ptr_eq(&parent.children()[0], &second));
    assert!(Rc::ptr_eq(&parent.children()[1], &first));
}

#[test]
fn test_focus_management() {
    let mut manager = WindowManager::new();
    let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let window2 = manager.create_window(None, 100, 100, 100, 100).unwrap();
    window1
        .borrow_mut()
        .set_system_callback(|_, msg, data1, data2| match msg {
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            _ => WindowMsgHandled::Ignored,
        });
    window2
        .borrow_mut()
        .set_system_callback(|_, msg, data1, data2| match msg {
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            _ => WindowMsgHandled::Ignored,
        });

    assert!(manager.get_focus().is_none());

    manager.set_focus(Some(&window1)).unwrap();
    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window1, &focused));

    manager.set_focus(Some(&window2)).unwrap();
    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window2, &focused));

    manager.set_focus(None).unwrap();
    assert!(manager.get_focus().is_none());
}

#[test]
fn test_focus_requires_input_focus_acceptance() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    manager.set_focus(Some(&window)).unwrap();

    assert!(manager.get_focus().is_none());
}

#[test]
fn set_focus_does_not_send_lost_when_refocusing_same_window_like_cpp() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let focus_messages = Rc::new(RefCell::new(Vec::new()));

    {
        let focus_messages = Rc::clone(&focus_messages);
        window
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                if msg == WindowMessage::InputFocus {
                    focus_messages.borrow_mut().push(data1);
                    return write_input_focus_response(data1, data2, data1 != 0);
                }
                WindowMsgHandled::Ignored
            });
    }

    manager.set_focus(Some(&window)).unwrap();
    manager.set_focus(Some(&window)).unwrap();

    assert_eq!(focus_messages.borrow().as_slice(), &[1, 1]);
    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window, &focused));
}

#[test]
fn set_focus_no_focus_window_preserves_existing_focus_like_cpp() {
    let mut manager = WindowManager::new();
    let focused = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let no_focus = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let focus_messages = Rc::new(RefCell::new(Vec::new()));

    {
        let focus_messages = Rc::clone(&focus_messages);
        focused
            .borrow_mut()
            .set_system_callback(move |_, msg, data1, data2| {
                if msg == WindowMessage::InputFocus {
                    focus_messages.borrow_mut().push(data1);
                    return write_input_focus_response(data1, data2, data1 != 0);
                }
                WindowMsgHandled::Ignored
            });
    }
    no_focus
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::NO_FOCUS);

    manager.set_focus(Some(&focused)).unwrap();
    manager.set_focus(Some(&no_focus)).unwrap();

    assert_eq!(focus_messages.borrow().as_slice(), &[1]);
    let current = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&focused, &current));
}

#[test]
fn test_focus_acceptance_can_come_from_parent() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 20, 20)
        .unwrap();
    parent
        .borrow_mut()
        .set_system_callback(|_, msg, data1, data2| match msg {
            WindowMessage::InputFocus if data1 != 0 => {
                write_input_focus_response(data1, data2, true)
            }
            _ => WindowMsgHandled::Ignored,
        });

    manager.set_focus(Some(&child)).unwrap();

    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&child, &focused));
}

#[test]
fn process_key_event_passes_key_and_state_to_parent_until_handled() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 20, 20)
        .unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));

    parent
        .borrow_mut()
        .set_system_callback(|_, msg, data1, data2| match msg {
            WindowMessage::InputFocus if data1 != 0 => {
                write_input_focus_response(data1, data2, true)
            }
            _ => WindowMsgHandled::Ignored,
        });

    child
        .borrow_mut()
        .set_input_callback(|_, msg, data1, data2| {
            if msg == WindowMessage::Char {
                assert_eq!((data1, data2), (13, 0x02));
            }
            WindowMsgHandled::Ignored
        });

    {
        let seen = Rc::clone(&seen);
        parent
            .borrow_mut()
            .set_input_callback(move |_, msg, data1, data2| {
                seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
    }

    manager.set_focus(Some(&child)).unwrap();

    assert_eq!(
        manager.process_key_event(13, 0x02),
        WindowInputReturnCode::Used
    );
    assert_eq!(seen.borrow().as_slice(), &[(WindowMessage::Char, 13, 0x02)]);
    assert_eq!(
        manager.process_key_event(0, 0x02),
        WindowInputReturnCode::NotUsed
    );
}

#[test]
fn test_mouse_capture() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    assert!(manager.get_capture().is_none());

    manager.capture_mouse(&window).unwrap();
    let captured = manager.get_capture().unwrap();
    assert!(Rc::ptr_eq(&window, &captured));

    manager.release_capture(&window).unwrap();
    assert!(manager.get_capture().is_none());
}

#[test]
fn release_capture_is_idempotent_like_cpp() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let other = manager.create_window(None, 100, 0, 100, 100).unwrap();

    assert_eq!(manager.release_capture(&window), Ok(()));

    manager.capture_mouse(&window).unwrap();
    assert_eq!(manager.release_capture(&other), Ok(()));
    let captured = manager.get_capture().unwrap();
    assert!(Rc::ptr_eq(&window, &captured));

    assert_eq!(manager.release_capture(&window), Ok(()));
    assert!(manager.get_capture().is_none());
}

#[test]
fn mouse_up_does_not_auto_release_capture_like_cpp() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    window
        .borrow_mut()
        .set_input_callback(|_, msg, _, _| match msg {
            WindowMessage::LeftUp => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        });

    manager.capture_mouse(&window).unwrap();
    let result = manager.process_mouse_event(WindowMessage::LeftUp, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(Rc::ptr_eq(&manager.get_capture().unwrap(), &window));
    assert!(manager.capture_flags.contains(CaptureFlags::MOUSE));
}

#[test]
fn mouse_up_outside_capture_does_not_auto_release_capture_like_cpp() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    manager.capture_mouse(&window).unwrap();
    let result = manager.process_mouse_event(WindowMessage::LeftUp, 500, 500, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    assert!(Rc::ptr_eq(&manager.get_capture().unwrap(), &window));
    assert!(manager.capture_flags.contains(CaptureFlags::MOUSE));
}

#[test]
fn process_mouse_event_sets_window_tooltip_like_cpp() {
    let _mouse_guard = lock_test_mouse();
    with_mouse(|mouse| mouse.set_cursor_tooltip("Stale".to_string(), Some(0), None, None));

    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    {
        let mut window = window.borrow_mut();
        window.set_tooltip("Window tip");
        window.instance_data_mut().tooltip_delay = 123;
    }

    let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    with_mouse(|mouse| {
        let state = mouse.cursor_tooltip_state();
        assert_eq!(state.tooltip_text, "Window tip");
        assert_eq!(state.tooltip_delay_override_ms, 123);
        assert!(!state.is_tooltip_empty);
    });
}

#[test]
fn process_mouse_event_clears_stale_tooltip_without_tooltip_window_like_cpp() {
    let _mouse_guard = lock_test_mouse();
    with_mouse(|mouse| mouse.set_cursor_tooltip("Stale".to_string(), Some(0), None, None));

    let mut manager = WindowManager::new();

    let result = manager.process_mouse_event(WindowMessage::MousePos, 500, 500, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    with_mouse(|mouse| {
        let state = mouse.cursor_tooltip_state();
        assert_eq!(state.tooltip_text, "");
        assert!(state.is_tooltip_empty);
    });
}

#[test]
fn process_mouse_event_reads_disabled_window_tooltip_like_cpp() {
    let _mouse_guard = lock_test_mouse();
    with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));

    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    {
        let mut window = window.borrow_mut();
        window.set_tooltip("Disabled tip");
        window.enable(false).unwrap();
    }

    let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    with_mouse(|mouse| {
        let state = mouse.cursor_tooltip_state();
        assert_eq!(state.tooltip_text, "Disabled tip");
        assert!(!state.is_tooltip_empty);
    });
}

#[test]
fn process_mouse_event_reads_no_input_child_tooltip_like_cpp() {
    let _mouse_guard = lock_test_mouse();
    with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));

    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 40, 40)
        .unwrap();
    {
        let mut child = child.borrow_mut();
        child.set_status(WindowStatus::NO_INPUT);
        child.set_tooltip("No input child tip");
    }

    let result = manager.process_mouse_event(WindowMessage::MousePos, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    with_mouse(|mouse| {
        let state = mouse.cursor_tooltip_state();
        assert_eq!(state.tooltip_text, "No input child tip");
        assert!(!state.is_tooltip_empty);
    });
}

#[test]
fn tooltip_scan_skips_disabled_tooltipless_overlay_like_cpp() {
    let _mouse_guard = lock_test_mouse();
    with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));

    let mut manager = WindowManager::new();
    let lower = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let overlay = manager.create_window(None, 0, 0, 100, 100).unwrap();

    lower.borrow_mut().set_tooltip("Lower tip");
    overlay.borrow_mut().enable(false).unwrap();
    manager.bring_window_forward(&overlay);

    let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    with_mouse(|mouse| {
        let state = mouse.cursor_tooltip_state();
        assert_eq!(state.tooltip_text, "Lower tip");
        assert!(!state.is_tooltip_empty);
    });
}

#[test]
fn process_mouse_event_only_clears_tooltip_during_capture_like_cpp() {
    let _mouse_guard = lock_test_mouse();
    with_mouse(|mouse| mouse.set_cursor_tooltip("Stale".to_string(), Some(0), None, None));

    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    window.borrow_mut().set_tooltip("Captured tip");
    manager.capture_mouse(&window).unwrap();

    let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    with_mouse(|mouse| {
        let state = mouse.cursor_tooltip_state();
        assert_eq!(state.tooltip_text, "");
        assert!(state.is_tooltip_empty);
    });
}

#[test]
fn mouse_pos_does_not_forward_to_focus_window_like_cpp() {
    let mut manager = WindowManager::new();
    let focused = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let hovered = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let focused_seen = Rc::new(RefCell::new(0));
    let hovered_seen = Rc::new(RefCell::new(0));

    focused
        .borrow_mut()
        .set_system_callback(|_, msg, data1, data2| match msg {
            WindowMessage::InputFocus if data1 != 0 => {
                write_input_focus_response(data1, data2, true)
            }
            _ => WindowMsgHandled::Ignored,
        });
    {
        let focused_seen = Rc::clone(&focused_seen);
        focused
            .borrow_mut()
            .set_input_callback(move |_, msg, _, _| {
                if msg == WindowMessage::MousePos {
                    *focused_seen.borrow_mut() += 1;
                }
                WindowMsgHandled::Handled
            });
    }
    {
        let hovered_seen = Rc::clone(&hovered_seen);
        hovered
            .borrow_mut()
            .set_input_callback(move |_, msg, _, _| {
                if msg == WindowMessage::MousePos {
                    *hovered_seen.borrow_mut() += 1;
                }
                WindowMsgHandled::Handled
            });
    }

    manager.set_focus(Some(&focused)).unwrap();

    let result = manager.process_mouse_event(WindowMessage::MousePos, 210, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert_eq!(*hovered_seen.borrow(), 1);
    assert_eq!(*focused_seen.borrow(), 0);

    let result = manager.process_mouse_event(WindowMessage::MousePos, 500, 500, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    assert_eq!(*hovered_seen.borrow(), 1);
    assert_eq!(*focused_seen.borrow(), 0);
}

#[test]
fn mouse_region_updates_on_button_events_like_cpp() {
    let mut manager = WindowManager::new();
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        first.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if matches!(
                msg,
                WindowMessage::MouseEntering | WindowMessage::MouseLeaving
            ) {
                seen.borrow_mut().push(("first", msg));
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }
    {
        let seen = Rc::clone(&seen);
        second.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if matches!(
                msg,
                WindowMessage::MouseEntering | WindowMessage::MouseLeaving
            ) {
                seen.borrow_mut().push(("second", msg));
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }

    manager.process_mouse_event(WindowMessage::LeftDown, 10, 10, 0);
    manager.process_mouse_event(WindowMessage::LeftDown, 210, 10, 0);

    assert_eq!(
        seen.borrow().as_slice(),
        &[
            ("first", WindowMessage::MouseEntering),
            ("first", WindowMessage::MouseLeaving),
            ("second", WindowMessage::MouseEntering)
        ]
    );
}

#[test]
fn capture_region_change_does_not_send_leaving_to_captor_like_cpp() {
    let mut manager = WindowManager::new();
    let captor = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&captor), 10, 10, 40, 40)
        .unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        captor.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if matches!(
                msg,
                WindowMessage::MouseEntering | WindowMessage::MouseLeaving
            ) {
                seen.borrow_mut().push(("captor", msg));
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }
    {
        let seen = Rc::clone(&seen);
        child.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if matches!(
                msg,
                WindowMessage::MouseEntering | WindowMessage::MouseLeaving
            ) {
                seen.borrow_mut().push(("child", msg));
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }

    manager.current_mouse_region = Some(Rc::downgrade(&captor));
    manager.capture_mouse(&captor).unwrap();
    manager.process_mouse_event(WindowMessage::MousePos, 20, 20, 0);

    assert_eq!(
        seen.borrow().as_slice(),
        &[("child", WindowMessage::MouseEntering)]
    );
}

#[test]
fn mouse_input_bubbles_to_parent_until_handled_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 40, 40)
        .unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        child.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                seen.borrow_mut().push(("child", msg));
            }
            WindowMsgHandled::Ignored
        });
    }
    {
        let seen = Rc::clone(&seen);
        parent.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                seen.borrow_mut().push(("parent", msg));
            }
            WindowMsgHandled::Handled
        });
    }

    let result = manager.process_mouse_event(WindowMessage::LeftUp, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            ("child", WindowMessage::LeftUp),
            ("parent", WindowMessage::LeftUp)
        ]
    );
}

#[test]
fn left_down_grabs_parent_that_handles_bubbled_input_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 40, 40)
        .unwrap();

    child
        .borrow_mut()
        .set_input_callback(|_, _, _, _| WindowMsgHandled::Ignored);
    parent
        .borrow_mut()
        .set_input_callback(|_, msg, _, _| match msg {
            WindowMessage::LeftDown => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        });

    let result = manager.process_mouse_event(WindowMessage::LeftDown, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(Rc::ptr_eq(&manager.get_grab_window().unwrap(), &parent));
}

#[test]
fn captured_mouse_input_bubbles_only_to_captor_like_cpp() {
    let mut manager = WindowManager::new();
    let root = manager.create_window(None, 0, 0, 200, 200).unwrap();
    let captor = manager.create_window(Some(&root), 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&captor), 10, 10, 40, 40)
        .unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        child.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                seen.borrow_mut().push(("child", msg));
            }
            WindowMsgHandled::Ignored
        });
    }
    {
        let seen = Rc::clone(&seen);
        captor.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                seen.borrow_mut().push(("captor", msg));
            }
            WindowMsgHandled::Ignored
        });
    }
    {
        let seen = Rc::clone(&seen);
        root.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                seen.borrow_mut().push(("root", msg));
            }
            WindowMsgHandled::Handled
        });
    }

    manager.capture_mouse(&captor).unwrap();
    let result = manager.process_mouse_event(WindowMessage::LeftUp, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            ("child", WindowMessage::LeftUp),
            ("captor", WindowMessage::LeftUp)
        ]
    );
}

#[test]
fn captured_mouse_input_can_target_no_input_child_like_cpp() {
    let mut manager = WindowManager::new();
    let captor = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&captor), 10, 10, 40, 40)
        .unwrap();
    child
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::NO_INPUT);
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        child.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                seen.borrow_mut().push("child");
            }
            WindowMsgHandled::Handled
        });
    }
    {
        let seen = Rc::clone(&seen);
        captor.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                seen.borrow_mut().push("captor");
            }
            WindowMsgHandled::Handled
        });
    }

    manager.capture_mouse(&captor).unwrap();
    let result = manager.process_mouse_event(WindowMessage::LeftUp, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert_eq!(seen.borrow().as_slice(), &["child"]);
}

#[test]
fn mouse_region_enter_reaches_routed_no_input_window_like_cpp() {
    let mut manager = WindowManager::new();
    let captor = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&captor), 10, 10, 40, 40)
        .unwrap();
    child
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::NO_INPUT);
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = Rc::clone(&seen);
        child.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if matches!(msg, WindowMessage::MousePos | WindowMessage::MouseEntering) {
                seen.borrow_mut().push(msg);
            }
            WindowMsgHandled::Ignored
        });
    }

    manager.capture_mouse(&captor).unwrap();
    let result = manager.process_mouse_event(WindowMessage::MousePos, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    assert_eq!(
        seen.borrow().as_slice(),
        &[WindowMessage::MousePos, WindowMessage::MouseEntering]
    );
}

#[test]
fn handled_mouse_down_outside_lone_window_closes_it_like_cpp() {
    let mut manager = WindowManager::new();
    let lone = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let outside = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let close_count = Rc::new(RefCell::new(0));

    {
        let close_count = Rc::clone(&close_count);
        lone.borrow_mut().set_system_callback(move |_, msg, _, _| {
            if msg == WindowMessage::User(16389) {
                *close_count.borrow_mut() += 1;
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }
    outside
        .borrow_mut()
        .set_input_callback(|_, msg, _, _| match msg {
            WindowMessage::LeftDown => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        });

    manager.set_lone_window(Some(&lone));
    let result = manager.process_mouse_event(WindowMessage::LeftDown, 210, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(manager.lone_window.is_none());
    assert_eq!(*close_count.borrow(), 1);
}

#[test]
fn lone_window_itself_is_not_its_own_child_like_cpp() {
    let mut manager = WindowManager::new();
    let lone = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let close_count = Rc::new(RefCell::new(0));

    {
        let close_count = Rc::clone(&close_count);
        lone.borrow_mut().set_system_callback(move |_, msg, _, _| {
            if msg == WindowMessage::User(16389) {
                *close_count.borrow_mut() += 1;
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }
    lone.borrow_mut()
        .set_input_callback(|_, msg, _, _| match msg {
            WindowMessage::LeftDown => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        });

    manager.set_lone_window(Some(&lone));
    let result = manager.process_mouse_event(WindowMessage::LeftDown, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(manager.lone_window.is_none());
    assert_eq!(*close_count.borrow(), 1);
}

#[test]
fn strict_lone_window_child_keeps_lone_window_open_like_cpp() {
    let mut manager = WindowManager::new();
    let lone = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager.create_window(Some(&lone), 10, 10, 40, 40).unwrap();
    let close_count = Rc::new(RefCell::new(0));

    {
        let close_count = Rc::clone(&close_count);
        lone.borrow_mut().set_system_callback(move |_, msg, _, _| {
            if msg == WindowMessage::User(16389) {
                *close_count.borrow_mut() += 1;
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }
    child
        .borrow_mut()
        .set_input_callback(|_, msg, _, _| match msg {
            WindowMessage::LeftDown => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        });

    manager.set_lone_window(Some(&lone));
    let result = manager.process_mouse_event(WindowMessage::LeftDown, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(manager.lone_window.is_some());
    assert_eq!(*close_count.borrow(), 0);
}

#[test]
fn hide_window_clears_runtime_references_for_window_and_children_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 20, 20)
        .unwrap();

    manager.keyboard_focus = Some(Rc::downgrade(&child));
    manager.capture_mouse(&child).unwrap();

    manager.hide_window(&parent, true).unwrap();

    assert!(parent.borrow().is_hidden());
    assert!(manager.get_focus().is_none());
    assert!(manager.get_capture().is_none());
    assert!(!manager.capture_flags.contains(CaptureFlags::MOUSE));
}

#[test]
fn hide_window_unsets_modal_head_like_cpp() {
    let mut manager = WindowManager::new();
    let bottom = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let top = manager.create_window(None, 100, 0, 100, 100).unwrap();

    manager.set_modal(bottom.clone()).unwrap();
    manager.set_modal(top.clone()).unwrap();

    manager.hide_window(&top, true).unwrap();

    assert!(top.borrow().is_hidden());
    assert!(Rc::ptr_eq(
        &manager.modal_stack.as_ref().unwrap().window,
        &bottom
    ));
}

#[test]
fn layout_hide_routes_through_manager_side_effects_like_cpp() {
    let window = with_window_manager(|manager| {
        manager.destroy_all_windows();
        manager.update();
        let layout = manager.create_layout("test_layout.wnd".to_string());
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus if data1 != 0 => {
                    write_input_focus_response(data1, data2, true)
                }
                _ => WindowMsgHandled::Ignored,
            });
        layout.borrow_mut().add_window(window.clone());
        manager.set_focus(Some(&window)).unwrap();
        manager.capture_mouse(&window).unwrap();

        // Re-enters `with_window_manager`; hide is queued and flushed when
        // this outer borrow returns (no overlapping `&mut`).
        layout.borrow().hide(true);
        window
    });

    assert!(window.borrow().is_hidden());
    with_window_manager(|manager| {
        assert!(manager.get_focus().is_none());
        assert!(manager.get_capture().is_none());
        assert!(!manager.capture_flags.contains(CaptureFlags::MOUSE));
        manager.destroy_all_windows();
        manager.update();
    });
}

#[test]
fn direct_window_hide_clears_runtime_references_like_cpp_win_hide() {
    let parent = with_window_manager(|manager| {
        manager.destroy_all_windows();
        manager.update();
        let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let child = manager
            .create_window(Some(&parent), 10, 10, 20, 20)
            .unwrap();
        child
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus if data1 != 0 => {
                    write_input_focus_response(data1, data2, true)
                }
                _ => WindowMsgHandled::Ignored,
            });

        manager.set_focus(Some(&child)).unwrap();
        manager.capture_mouse(&child).unwrap();
        manager.set_modal(parent.clone()).unwrap();

        // `GameWindow::hide` re-enters `with_window_manager`. Manager-side
        // focus/capture/modal cleanup is queued and flushed on return.
        parent.borrow_mut().hide(true).unwrap();
        parent
    });

    assert!(parent.borrow().is_hidden());
    with_window_manager(|manager| {
        assert!(manager.get_focus().is_none());
        assert!(manager.get_capture().is_none());
        assert!(manager.modal_stack.is_none());
        assert!(!manager.capture_flags.contains(CaptureFlags::MOUSE));
        manager.destroy_all_windows();
        manager.update();
    });
}

#[test]
fn test_modal_windows() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    manager.set_modal(window.clone()).unwrap();
    // Modal stack would be tested here, but the current implementation
    // doesn't provide easy access to check the modal stack state

    manager.unset_modal(&window).unwrap();
}

#[test]
fn set_modal_rejects_child_windows_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 20, 20)
        .unwrap();

    assert_eq!(manager.set_modal(child), Err(WindowError::InvalidParameter));
    assert!(manager.modal_stack.is_none());
}

#[test]
fn unset_modal_rejects_non_top_windows_like_cpp() {
    let mut manager = WindowManager::new();
    let bottom = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let top = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let never_modal = manager.create_window(None, 200, 0, 100, 100).unwrap();

    assert_eq!(
        manager.unset_modal(&never_modal),
        Err(WindowError::GeneralFailure)
    );

    manager.set_modal(bottom.clone()).unwrap();
    manager.set_modal(top.clone()).unwrap();

    assert_eq!(
        manager.unset_modal(&bottom),
        Err(WindowError::GeneralFailure)
    );
    assert!(Rc::ptr_eq(
        &manager.modal_stack.as_ref().unwrap().window,
        &top
    ));

    assert_eq!(manager.unset_modal(&top), Ok(()));
    assert!(Rc::ptr_eq(
        &manager.modal_stack.as_ref().unwrap().window,
        &bottom
    ));
}

#[test]
fn new_root_windows_insert_behind_modal_roots_like_cpp() {
    let mut manager = WindowManager::new();
    let normal = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let modal = manager.create_window(None, 100, 0, 100, 100).unwrap();

    manager.set_modal(modal.clone()).unwrap();
    let later = manager.create_window(None, 200, 0, 100, 100).unwrap();

    assert!(Rc::ptr_eq(&manager.root_windows[0], &modal));
    assert!(Rc::ptr_eq(&manager.root_windows[1], &later));
    assert!(Rc::ptr_eq(&manager.root_windows[2], &normal));
}

#[test]
fn root_sibling_links_follow_window_list_like_cpp() {
    let mut manager = WindowManager::new();
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let third = manager.create_window(None, 200, 0, 100, 100).unwrap();

    assert!(third.borrow().get_prev_sibling().is_none());
    assert!(Rc::ptr_eq(
        &third.borrow().get_next_sibling().unwrap(),
        &second
    ));
    assert!(Rc::ptr_eq(
        &second.borrow().get_prev_sibling().unwrap(),
        &third
    ));
    assert!(Rc::ptr_eq(
        &second.borrow().get_next_sibling().unwrap(),
        &first
    ));
    assert!(Rc::ptr_eq(
        &first.borrow().get_prev_sibling().unwrap(),
        &second
    ));
    assert!(first.borrow().get_next_sibling().is_none());

    manager.bring_window_forward(&first);

    assert!(first.borrow().get_prev_sibling().is_none());
    assert!(Rc::ptr_eq(
        &first.borrow().get_next_sibling().unwrap(),
        &third
    ));
    assert!(Rc::ptr_eq(
        &third.borrow().get_prev_sibling().unwrap(),
        &first
    ));
}

#[test]
fn bring_window_forward_moves_child_to_head_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let first = manager.create_window(Some(&parent), 0, 0, 20, 20).unwrap();
    let second = manager.create_window(Some(&parent), 20, 0, 20, 20).unwrap();

    manager.bring_window_forward(&second);

    let parent = parent.borrow();
    assert!(Rc::ptr_eq(&parent.children()[0], &second));
    assert!(Rc::ptr_eq(&parent.children()[1], &first));
}

#[test]
fn child_sibling_links_follow_parent_child_list_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let first = manager.create_window(Some(&parent), 0, 0, 20, 20).unwrap();
    let second = manager.create_window(Some(&parent), 20, 0, 20, 20).unwrap();
    let third = manager.create_window(Some(&parent), 40, 0, 20, 20).unwrap();

    assert!(third.borrow().get_prev_sibling().is_none());
    assert!(Rc::ptr_eq(
        &third.borrow().get_next_sibling().unwrap(),
        &second
    ));
    assert!(Rc::ptr_eq(
        &second.borrow().get_prev_sibling().unwrap(),
        &third
    ));
    assert!(Rc::ptr_eq(
        &second.borrow().get_next_sibling().unwrap(),
        &first
    ));
    assert!(Rc::ptr_eq(
        &first.borrow().get_prev_sibling().unwrap(),
        &second
    ));
    assert!(first.borrow().get_next_sibling().is_none());

    manager.set_window_parent(&second, None).unwrap();

    assert!(second.borrow().get_prev_sibling().is_none());
    assert!(Rc::ptr_eq(
        &second.borrow().get_next_sibling().unwrap(),
        &parent
    ));
    assert!(third.borrow().get_prev_sibling().is_none());
    assert!(Rc::ptr_eq(
        &third.borrow().get_next_sibling().unwrap(),
        &first
    ));
    assert!(Rc::ptr_eq(
        &first.borrow().get_prev_sibling().unwrap(),
        &third
    ));
    assert!(first.borrow().get_next_sibling().is_none());
}

#[test]
fn bring_window_forward_moves_root_to_head_like_cpp() {
    let mut manager = WindowManager::new();
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 100, 0, 100, 100).unwrap();

    manager.bring_window_forward(&first);

    assert!(Rc::ptr_eq(&manager.root_windows[0], &first));
    assert!(Rc::ptr_eq(&manager.root_windows[1], &second));
}

#[test]
fn bring_window_forward_updates_layout_order_like_cpp() {
    let mut manager = WindowManager::new();
    let layout = Rc::new(RefCell::new(WindowLayout::new("test.wnd".to_string())));
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 100, 0, 100, 100).unwrap();
    first.borrow_mut().set_layout(Some(&layout));
    second.borrow_mut().set_layout(Some(&layout));
    layout.borrow_mut().add_window(first.clone());
    layout.borrow_mut().add_window(second.clone());

    manager.bring_window_forward(&second);

    let layout_ref = layout.borrow();
    assert!(Rc::ptr_eq(&layout_ref.windows()[0], &second));
    assert!(Rc::ptr_eq(&layout_ref.windows()[1], &first));
}

#[test]
fn activate_window_brings_to_top_and_unhides_like_cpp() {
    let mut manager = WindowManager::new();
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 100, 0, 100, 100).unwrap();
    first.borrow_mut().hide(true).unwrap();

    manager.activate_window(&first).unwrap();

    assert!(first.borrow().get_status().contains(WindowStatus::ACTIVE));
    assert!(!first.borrow().is_hidden());
    assert!(Rc::ptr_eq(&manager.root_windows[0], &first));
    assert!(Rc::ptr_eq(&manager.root_windows[1], &second));
}

#[test]
fn left_up_on_grab_window_clears_active_and_grab_like_cpp() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    window.borrow_mut().set_status(WindowStatus::ACTIVE);
    manager.set_grab_window(Some(&window));

    let result = manager.process_mouse_event(WindowMessage::LeftUp, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(!window.borrow().get_status().contains(WindowStatus::ACTIVE));
    assert!(manager.get_grab_window().is_none());
}

#[test]
fn left_up_on_grab_window_sends_release_to_grabbed_window_like_cpp() {
    let mut manager = WindowManager::new();
    let grabbed = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let other = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let grabbed_seen = Rc::new(RefCell::new(false));
    let other_seen = Rc::new(RefCell::new(false));

    {
        let grabbed_seen = grabbed_seen.clone();
        grabbed
            .borrow_mut()
            .set_input_callback(move |_, msg, _, _| {
                if msg == WindowMessage::LeftUp {
                    *grabbed_seen.borrow_mut() = true;
                    WindowMsgHandled::Handled
                } else {
                    WindowMsgHandled::Ignored
                }
            });
    }
    {
        let other_seen = other_seen.clone();
        other.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftUp {
                *other_seen.borrow_mut() = true;
            }
            WindowMsgHandled::Handled
        });
    }

    manager.set_grab_window(Some(&grabbed));
    let result = manager.process_mouse_event(WindowMessage::LeftUp, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(*grabbed_seen.borrow());
    assert!(!*other_seen.borrow());
}

#[test]
fn left_down_sets_grab_only_when_handled_like_cpp() {
    let mut manager = WindowManager::new();
    let handled = manager.create_window(None, 0, 0, 100, 100).unwrap();
    handled.borrow_mut().set_input_callback(|_, msg, _, _| {
        if msg == WindowMessage::LeftDown {
            WindowMsgHandled::Handled
        } else {
            WindowMsgHandled::Ignored
        }
    });

    let result = manager.process_mouse_event(WindowMessage::LeftDown, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(Rc::ptr_eq(&manager.get_grab_window().unwrap(), &handled));
    assert!(manager.get_capture().is_none());

    let mut manager = WindowManager::new();
    let ignored = manager.create_window(None, 0, 0, 100, 100).unwrap();
    ignored
        .borrow_mut()
        .set_input_callback(|_, _, _, _| WindowMsgHandled::Ignored);

    let result = manager.process_mouse_event(WindowMessage::LeftDown, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    assert!(manager.get_grab_window().is_none());
    assert!(manager.get_capture().is_none());
}

#[test]
fn captured_mouse_event_clears_existing_grab_like_cpp() {
    let mut manager = WindowManager::new();
    let captured = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let grabbed = manager.create_window(None, 200, 0, 100, 100).unwrap();

    manager.capture_mouse(&captured).unwrap();
    manager.set_grab_window(Some(&grabbed));

    let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    assert!(manager.get_grab_window().is_none());
}

#[test]
fn left_drag_on_grab_window_routes_to_grabbed_window_like_cpp() {
    let mut manager = WindowManager::new();
    let grabbed = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let other = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let grabbed_seen = Rc::new(RefCell::new(false));
    let other_seen = Rc::new(RefCell::new(false));

    {
        let grabbed_seen = grabbed_seen.clone();
        grabbed
            .borrow_mut()
            .set_input_callback(move |_, msg, _, _| {
                if msg == WindowMessage::LeftDrag {
                    *grabbed_seen.borrow_mut() = true;
                }
                WindowMsgHandled::Handled
            });
    }
    {
        let other_seen = other_seen.clone();
        other.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftDrag {
                *other_seen.borrow_mut() = true;
            }
            WindowMsgHandled::Handled
        });
    }

    manager.set_grab_window(Some(&grabbed));
    let result = manager.process_mouse_event(WindowMessage::LeftDrag, 210, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(*grabbed_seen.borrow());
    assert!(!*other_seen.borrow());
    assert!(Rc::ptr_eq(&manager.get_grab_window().unwrap(), &grabbed));
}

#[test]
fn non_drag_mouse_event_on_grab_window_is_consumed_like_cpp() {
    let mut manager = WindowManager::new();
    let grabbed = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let other = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let grabbed_seen = Rc::new(RefCell::new(false));
    let other_seen = Rc::new(RefCell::new(false));

    {
        let grabbed_seen = grabbed_seen.clone();
        grabbed
            .borrow_mut()
            .set_input_callback(move |_, msg, _, _| {
                if msg == WindowMessage::RightDown {
                    *grabbed_seen.borrow_mut() = true;
                }
                WindowMsgHandled::Handled
            });
    }
    {
        let other_seen = other_seen.clone();
        other.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::RightDown {
                *other_seen.borrow_mut() = true;
            }
            WindowMsgHandled::Handled
        });
    }

    manager.set_grab_window(Some(&grabbed));
    let result = manager.process_mouse_event(WindowMessage::RightDown, 210, 10, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(!*grabbed_seen.borrow());
    assert!(!*other_seen.borrow());
    assert!(Rc::ptr_eq(&manager.get_grab_window().unwrap(), &grabbed));
}

#[test]
fn left_drag_moves_grabbed_draggable_window_like_cpp() {
    let mut manager = WindowManager::new();
    let grabbed = manager.create_window(None, 10, 20, 100, 80).unwrap();
    grabbed.borrow_mut().set_status(WindowStatus::DRAGABLE);
    manager.set_grab_window(Some(&grabbed));

    let result =
        manager.process_mouse_event_with_delta(WindowMessage::LeftDrag, 30, 40, 0, Some((15, -5)));

    assert_eq!(result, WindowInputReturnCode::Used);
    assert_eq!(grabbed.borrow().get_position(), (25, 15));
}

#[test]
fn left_drag_clips_grabbed_draggable_window_to_parent_and_screen_like_cpp() {
    let mut manager = WindowManager::new();
    manager.set_screen_size(300, 200);
    let parent = manager.create_window(None, 0, 0, 150, 120).unwrap();
    let grabbed = manager
        .create_window(Some(&parent), 80, 70, 100, 80)
        .unwrap();
    grabbed.borrow_mut().set_status(WindowStatus::DRAGABLE);
    manager.set_grab_window(Some(&grabbed));

    let result = manager.process_mouse_event_with_delta(
        WindowMessage::LeftDrag,
        200,
        160,
        0,
        Some((100, 100)),
    );

    assert_eq!(result, WindowInputReturnCode::Used);
    assert_eq!(grabbed.borrow().get_position(), (50, 40));
}

#[test]
fn bring_layout_forward_moves_roots_to_head_like_cpp() {
    let mut manager = WindowManager::new();
    let background = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let first = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 200, 0, 100, 100).unwrap();
    let foreground = manager.create_window(None, 300, 0, 100, 100).unwrap();
    let mut layout = WindowLayout::new("test.wnd".to_string());
    layout.add_window(first.clone());
    layout.add_window(second.clone());

    manager.bring_layout_forward(&layout);

    assert!(Rc::ptr_eq(&manager.root_windows[0], &second));
    assert!(Rc::ptr_eq(&manager.root_windows[1], &first));
    assert!(Rc::ptr_eq(&manager.root_windows[2], &foreground));
    assert!(Rc::ptr_eq(&manager.root_windows[3], &background));
}

#[test]
fn bring_layout_forward_moves_children_to_head_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let first = manager.create_window(Some(&parent), 0, 0, 20, 20).unwrap();
    let second = manager.create_window(Some(&parent), 20, 0, 20, 20).unwrap();
    let foreground = manager.create_window(Some(&parent), 40, 0, 20, 20).unwrap();
    let mut layout = WindowLayout::new("test.wnd".to_string());
    layout.add_window(first.clone());
    layout.add_window(second.clone());

    manager.bring_layout_forward(&layout);

    let parent = parent.borrow();
    assert!(Rc::ptr_eq(&parent.children()[0], &second));
    assert!(Rc::ptr_eq(&parent.children()[1], &first));
    assert!(Rc::ptr_eq(&parent.children()[2], &foreground));
}

#[test]
fn get_window_by_id_returns_first_traversal_match_like_cpp() {
    let mut manager = WindowManager::new();
    let id = 42;
    let first = manager
        .create_window_with_id(None, 0, 0, 100, 100, id)
        .unwrap();
    let second = manager
        .create_window_with_id(None, 100, 0, 100, 100, id)
        .unwrap();

    let found = manager.get_window_by_id(id).unwrap();
    assert!(Rc::ptr_eq(&found, &second));

    manager.bring_window_forward(&first);

    let found = manager.get_window_by_id(id).unwrap();
    assert!(Rc::ptr_eq(&found, &first));
}

#[test]
fn os_mouse_dispatch_selects_push_button_when_shell_inactive() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let _lock = lock_test_mouse();
    crate::gui::shell::get_shell().set_shell_active(false);
    let clicked = Arc::new(AtomicBool::new(false));
    with_window_manager(|manager| {
        manager.reset();
        let window = manager.create_window(None, 10, 10, 80, 30).unwrap();
        let mut button = PushButton::new(99, 0, 0, 80, 30);
        button.set_triggers_on_mouse_down(true);
        let flag = Arc::clone(&clicked);
        button.set_callback(Box::new(move |_| flag.store(true, Ordering::SeqCst)));
        window
            .borrow_mut()
            .set_widget(WindowWidget::PushButton(button));
        assert!(
            window.borrow().point_in_window(20, 20),
            "test click must land inside created window"
        );
        let rc = manager.process_mouse_event(WindowMessage::LeftDown, 20, 20, 0);
        assert_eq!(rc, WindowInputReturnCode::Used);
    });
    assert!(
        clicked.load(Ordering::SeqCst),
        "OS LeftDown must hit-test the push button and fire GadgetSelected"
    );
    let wrapped = dispatch_os_mouse_to_window_manager(WindowMessage::LeftDown, 20, 20);
    assert_eq!(wrapped, WindowInputReturnCode::Used);
    crate::gui::shell::get_shell().set_shell_active(true);
}

#[test]
fn os_click_named_window_hit_tests_widget_tree_and_selects_owner() {
    use crate::gui::shell::main_menu::{
        dispatch_os_click_named_window, last_os_wnd_widget_tree_click_ok,
        reset_os_wnd_widget_tree_nav_for_tests,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let _lock = lock_test_mouse();
    reset_os_wnd_widget_tree_nav_for_tests();
    crate::gui::shell::get_shell().set_shell_active(false);
    let selected = Arc::new(AtomicBool::new(false));
    let button_name = "TestMenu.wnd:ButtonStart";
    with_window_manager(|manager| {
        manager.reset();
        let parent = manager.create_window(None, 0, 0, 200, 80).unwrap();
        let flag = Arc::clone(&selected);
        parent
            .borrow_mut()
            .set_system_callback(move |_, msg, _, _| {
                if msg == WindowMessage::GadgetSelected {
                    flag.store(true, Ordering::SeqCst);
                    WindowMsgHandled::Handled
                } else {
                    WindowMsgHandled::Ignored
                }
            });
        let id = manager
            .gogo_gadget_push_button(Some(&parent), (10, 10), (80, 30))
            .expect("push button");
        let button = manager.get_window_by_id(id).expect("button window");
        button.borrow_mut().set_name(button_name);
        let (sx, sy) = button.borrow().get_screen_position();
        let (w, h) = button.borrow().get_size();
        let hit = manager
            .get_window_under_cursor(sx + w / 2, sy + h / 2, false)
            .expect("hit");
        assert!(
            Rc::ptr_eq(&hit, &button),
            "center of named gadget must be the get_window_under_cursor hit"
        );
    });
    assert!(
        dispatch_os_click_named_window(button_name),
        "named click must hit-test the live gadget"
    );
    assert!(
        last_os_wnd_widget_tree_click_ok(),
        "LeftDown/Up must be consumed by the WND widget tree"
    );
    assert!(
        crate::gui::shell::main_menu::os_wnd_widget_tree_nav_ok(),
        "named dispatch must latch sticky wnd_widget_tree_nav"
    );
    assert!(
        selected.load(Ordering::SeqCst),
        "owner must receive GBM_SELECTED without simulate_*"
    );
    assert!(!dispatch_os_click_named_window("TestMenu.wnd:Missing"));
    assert!(!last_os_wnd_widget_tree_click_ok());
    crate::gui::shell::get_shell().set_shell_active(true);
}

#[test]
fn os_click_named_window_rejects_when_another_window_covers_hit() {
    use crate::gui::shell::main_menu::{
        dispatch_os_click_named_window, last_os_wnd_widget_tree_click_ok,
        reset_os_wnd_widget_tree_nav_for_tests,
    };

    let _lock = lock_test_mouse();
    reset_os_wnd_widget_tree_nav_for_tests();
    crate::gui::shell::get_shell().set_shell_active(false);
    with_window_manager(|manager| {
        manager.reset();
        let named = manager.create_window(None, 10, 10, 80, 30).unwrap();
        named.borrow_mut().set_name("Covered.wnd:Button");
        named
            .borrow_mut()
            .set_widget(WindowWidget::PushButton(PushButton::new(1, 0, 0, 80, 30)));
        let cover = manager.create_window(None, 10, 10, 80, 30).unwrap();
        cover.borrow_mut().set_status(WindowStatus::ABOVE);
        cover.borrow_mut().set_name("Covered.wnd:Blocker");
    });
    assert!(
        !dispatch_os_click_named_window("Covered.wnd:Button"),
        "named click must fail when get_window_under_cursor is not the named gadget"
    );
    assert!(!last_os_wnd_widget_tree_click_ok());
    crate::gui::shell::get_shell().set_shell_active(true);
}

#[test]
#[test]
fn os_wnd_widget_under_cursor_name_returns_enabled_gadget() {
    use crate::gui::shell::main_menu::os_wnd_widget_under_cursor_name;

    let _lock = lock_test_mouse();
    crate::gui::shell::get_shell().set_shell_active(false);
    with_window_manager(|manager| {
        manager.reset();
        let named = manager.create_window(None, 0, 0, 80, 30).unwrap();
        named.borrow_mut().set_name("MainMenu.wnd:ButtonSkirmish");
    });
    assert_eq!(
        os_wnd_widget_under_cursor_name(40, 15).as_deref(),
        Some("MainMenu.wnd:ButtonSkirmish")
    );
    assert_eq!(os_wnd_widget_under_cursor_name(400, 400), None);
}

#[test]
fn note_os_wnd_widget_tree_hit_latches_sticky_nav() {
    use crate::gui::shell::main_menu::{
        note_os_wnd_widget_tree_hit, os_wnd_widget_tree_nav_ok,
        reset_os_wnd_widget_tree_nav_for_tests,
    };

    let _lock = lock_test_mouse();
    reset_os_wnd_widget_tree_nav_for_tests();
    with_window_manager(|manager| {
        manager.reset();
        let _ = manager.create_window(None, 0, 0, 80, 30).unwrap();
    });
    assert!(note_os_wnd_widget_tree_hit(40, 15));
    assert!(os_wnd_widget_tree_nav_ok());
    assert!(!note_os_wnd_widget_tree_hit(400, 400));
    assert!(
        os_wnd_widget_tree_nav_ok(),
        "sticky nav must survive a later miss"
    );
}

#[test]
fn os_mouse_dispatch_consumed_when_shell_active() {
    let _lock = lock_test_mouse();
    crate::gui::shell::get_shell().set_shell_active(true);
    let rc = dispatch_os_mouse_to_window_manager(WindowMessage::MousePos, 4, 4);
    assert_eq!(rc, WindowInputReturnCode::Used);
}

#[test]
fn os_mouse_dispatch_skips_wm_when_mouse_locked_unless_scrolling_lmb() {
    // C++ WindowXlat.cpp:147-167 — locked view KEEP_MESSAGE except LMB
    // while TheInGameUI::isScrolling so ControlBar clicks still land.
    let _lock = lock_test_mouse();
    crate::gui::shell::get_shell().set_shell_active(false);
    crate::display::view::with_tactical_view(|view| view.set_mouse_lock(true));
    crate::helpers::TheInGameUI::set_scrolling(false);
    assert_eq!(
        dispatch_os_mouse_to_window_manager(WindowMessage::LeftDown, 20, 20),
        WindowInputReturnCode::NotUsed,
        "locked, not scrolling: skip WM (C++ KEEP_MESSAGE)"
    );

    crate::helpers::TheInGameUI::set_scrolling(true);
    with_window_manager(|manager| {
        manager.reset();
        let window = manager.create_window(None, 10, 10, 80, 30).unwrap();
        window
            .borrow_mut()
            .set_widget(WindowWidget::PushButton(PushButton::new(1, 0, 0, 80, 30)));
    });
    assert_eq!(
        dispatch_os_mouse_to_window_manager(WindowMessage::LeftDown, 20, 20),
        WindowInputReturnCode::Used,
        "locked + scrolling: LMB must still hit ControlBar"
    );
    assert_eq!(
        dispatch_os_mouse_to_window_manager(WindowMessage::RightDown, 20, 20),
        WindowInputReturnCode::NotUsed,
        "locked + scrolling: non-LMB still KEEP_MESSAGE"
    );

    crate::helpers::TheInGameUI::set_scrolling(false);
    crate::display::view::with_tactical_view(|view| view.set_mouse_lock(false));
    crate::gui::shell::get_shell().set_shell_active(true);
}

#[test]
fn os_key_dispatch_enter_reaches_focused_window_when_shell_inactive() {
    let _lock = lock_test_mouse();
    crate::gui::shell::get_shell().set_shell_active(false);
    let seen = Rc::new(RefCell::new(Vec::new()));
    with_window_manager(|manager| {
        manager.reset();
        let window = manager.create_window(None, 0, 0, 100, 40).unwrap();
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus if data1 != 0 => {
                    write_input_focus_response(data1, data2, true)
                }
                _ => WindowMsgHandled::Ignored,
            });
        {
            let seen = Rc::clone(&seen);
            window
                .borrow_mut()
                .set_input_callback(move |_, msg, data1, data2| {
                    seen.borrow_mut().push((msg, data1, data2));
                    WindowMsgHandled::Handled
                });
        }
        manager.set_focus(Some(&window)).unwrap();
        let rc = manager.process_key_event(13, 0x02);
        assert_eq!(rc, WindowInputReturnCode::Used);
    });
    let wrapped = dispatch_os_key_to_window_manager(13, 0x02);
    assert_eq!(wrapped, WindowInputReturnCode::Used);
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            (WindowMessage::Char, 13, 0x02),
            (WindowMessage::Char, 13, 0x02)
        ]
    );
    crate::gui::shell::get_shell().set_shell_active(true);
}

#[test]
fn os_key_dispatch_consumed_when_shell_active() {
    let _lock = lock_test_mouse();
    crate::gui::shell::get_shell().set_shell_active(true);
    let rc = dispatch_os_key_to_window_manager(0x1B, 0x01);
    assert_eq!(rc, WindowInputReturnCode::Used);
}

#[test]
fn find_window_from_id_continues_through_siblings_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let first = manager
        .create_window_with_id(Some(&parent), 0, 0, 20, 20, 10)
        .unwrap();
    let second = manager
        .create_window_with_id(Some(&parent), 20, 0, 20, 20, 20)
        .unwrap();

    let found = manager.find_window_from_id(&second, 10).unwrap();
    assert!(Rc::ptr_eq(&found, &first));
    assert!(manager.find_window_from_id(&first, 20).is_none());
}

#[test]
fn range_helpers_mutate_first_lookup_match_per_id_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let first = manager
        .create_window_with_id(Some(&parent), 0, 0, 20, 20, 10)
        .unwrap();
    let duplicate = manager
        .create_window_with_id(Some(&parent), 20, 0, 20, 20, 10)
        .unwrap();
    let next = manager
        .create_window_with_id(Some(&parent), 40, 0, 20, 20, 11)
        .unwrap();

    manager.hide_windows_in_range(&next, 10, 11, true);

    assert!(!first.borrow().is_hidden());
    assert!(duplicate.borrow().is_hidden());
    assert!(next.borrow().is_hidden());

    manager.enable_windows_in_range(&next, 10, 11, false);

    assert!(first.borrow().is_enabled());
    assert!(!duplicate.borrow().is_enabled());
    assert!(!next.borrow().is_enabled());
}

#[test]
fn get_window_under_cursor_prioritizes_above_normal_below_like_cpp() {
    let mut manager = WindowManager::new();
    let below = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let normal = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let above = manager.create_window(None, 0, 0, 100, 100).unwrap();

    below
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::BELOW);
    normal.borrow_mut().set_status_exact(WindowStatus::ENABLED);
    above
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::ABOVE);

    let found = manager.get_window_under_cursor(10, 10, false).unwrap();
    assert!(Rc::ptr_eq(&above, &found));

    above
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::ABOVE | WindowStatus::HIDDEN);

    let found = manager.get_window_under_cursor(10, 10, false).unwrap();
    assert!(Rc::ptr_eq(&normal, &found));
}

#[test]
fn get_window_under_cursor_uses_capture_before_roots_like_cpp() {
    let mut manager = WindowManager::new();
    let normal = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let captured = manager.create_window(None, 200, 0, 100, 100).unwrap();

    manager.capture_mouse(&captured).unwrap();

    let found = manager.get_window_under_cursor(10, 10, false).unwrap();
    assert!(Rc::ptr_eq(&captured, &found));
    assert!(!Rc::ptr_eq(&normal, &found));
}

#[test]
fn get_window_under_cursor_uses_grab_when_not_captured_like_cpp() {
    let mut manager = WindowManager::new();
    let normal = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let grabbed = manager.create_window(None, 200, 0, 100, 100).unwrap();

    manager.set_grab_window(Some(&grabbed));

    let found = manager.get_window_under_cursor(10, 10, false).unwrap();
    assert!(Rc::ptr_eq(&grabbed, &found));
    assert!(!Rc::ptr_eq(&normal, &found));
}

#[test]
fn get_window_under_cursor_prefers_capture_over_grab_like_cpp() {
    let mut manager = WindowManager::new();
    let captured = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let grabbed = manager.create_window(None, 200, 0, 100, 100).unwrap();

    manager.capture_mouse(&captured).unwrap();
    manager.set_grab_window(Some(&grabbed));

    let found = manager.get_window_under_cursor(250, 10, false).unwrap();
    assert!(Rc::ptr_eq(&captured, &found));
}

#[test]
fn get_window_under_cursor_returns_captured_child_like_cpp() {
    let mut manager = WindowManager::new();
    let captured = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&captured), 10, 10, 40, 40)
        .unwrap();

    manager.capture_mouse(&captured).unwrap();

    let found = manager.get_window_under_cursor(20, 20, false).unwrap();
    assert!(Rc::ptr_eq(&child, &found));
}

#[test]
fn get_window_under_cursor_returns_modal_when_outside_like_cpp() {
    let mut manager = WindowManager::new();
    let normal = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let modal = manager.create_window(None, 200, 0, 100, 100).unwrap();

    manager.set_modal(modal.clone()).unwrap();

    let found = manager.get_window_under_cursor(10, 10, false).unwrap();
    assert!(Rc::ptr_eq(&modal, &found));
    assert!(!Rc::ptr_eq(&normal, &found));
}

#[test]
fn get_window_under_cursor_discards_disabled_ignore_enabled_hit_like_cpp() {
    let mut manager = WindowManager::new();
    let disabled = manager.create_window(None, 0, 0, 100, 100).unwrap();

    disabled
        .borrow_mut()
        .set_status_exact(WindowStatus::empty());

    assert!(manager.get_window_under_cursor(10, 10, true).is_none());
}

#[test]
fn no_input_child_blocks_non_combo_parent_mouse_hit_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 40, 40)
        .unwrap();
    child.borrow_mut().set_status(WindowStatus::NO_INPUT);
    parent
        .borrow_mut()
        .set_input_callback(|_, msg, _, _| match msg {
            WindowMessage::LeftDown => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        });

    let result = manager.process_mouse_event(WindowMessage::LeftDown, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::NotUsed);
    assert!(manager.get_grab_window().is_none());
}

#[test]
fn no_input_combo_child_retargets_mouse_hit_to_combo_parent_like_cpp() {
    let mut manager = WindowManager::new();
    let combo = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager.create_window(Some(&combo), 10, 10, 40, 40).unwrap();
    let seen = Rc::new(RefCell::new(false));

    combo.borrow_mut().instance_data_mut().style = GWS_COMBO_BOX;
    child.borrow_mut().set_status(WindowStatus::NO_INPUT);
    {
        let seen = Rc::clone(&seen);
        combo.borrow_mut().set_input_callback(move |_, msg, _, _| {
            if msg == WindowMessage::LeftDown {
                *seen.borrow_mut() = true;
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }

    let result = manager.process_mouse_event(WindowMessage::LeftDown, 20, 20, 0);

    assert_eq!(result, WindowInputReturnCode::Used);
    assert!(*seen.borrow());
    assert!(Rc::ptr_eq(&manager.get_grab_window().unwrap(), &combo));
}

#[test]
fn create_slider_thumb_child_is_enabled_dragable_not_no_input() {
    let mut manager = WindowManager::new();
    let slider = manager.create_window(None, 0, 0, 120, 20).unwrap();
    slider.borrow_mut().instance_data_mut().style |= GWS_HORZ_SLIDER;
    slider
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::ACTIVE);

    manager
        .create_slider_thumb_child(&slider, &WindowLayoutDefinition::default())
        .unwrap();

    let thumb_id = slider.borrow().slider_thumb().expect("thumb child");
    let thumb = slider
        .borrow()
        .find_child_by_id(thumb_id)
        .expect("thumb window");
    let status = thumb.borrow().get_status();
    assert!(
        status.contains(WindowStatus::ENABLED),
        "C++ slider thumb is ENABLED"
    );
    assert!(
        status.contains(WindowStatus::DRAGABLE),
        "C++ slider thumb is DRAGABLE"
    );
    assert!(
        !status.contains(WindowStatus::NO_INPUT),
        "C++ slider thumb is not NO_INPUT"
    );
}

#[test]
fn get_window_under_cursor_walks_no_input_parent_to_enabled_child() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 40, 40)
        .unwrap();
    parent
        .borrow_mut()
        .set_status_exact(WindowStatus::ENABLED | WindowStatus::NO_INPUT);
    child.borrow_mut().set_status_exact(WindowStatus::ENABLED);

    let found = manager.get_window_under_cursor(20, 20, false).unwrap();
    assert!(
        Rc::ptr_eq(&child, &found),
        "winPointInChild must walk NO_INPUT parents before post-filter"
    );
}

#[test]
fn test_window_destruction() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let window_id = window.borrow().get_id();

    assert_eq!(manager.window_count, 1);
    assert!(manager.get_window_by_id(window_id).is_some());

    manager.destroy_window(window).unwrap();
    manager.update(); // Process destroy queue

    assert_eq!(manager.window_count, 0);
    assert!(manager.get_window_by_id(window_id).is_none());
}

#[test]
#[should_panic(expected = "winDestroy(): edit data should NOT be present!")]
fn destroy_window_rejects_editor_edit_data_like_cpp_debug_assert() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    window
        .borrow_mut()
        .set_edit_data(Some(GameWindowEditData::default()));

    manager.destroy_window(window).unwrap();
    manager.update();
}

#[test]
fn destroy_window_recursively_destroys_children_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 20, 20)
        .unwrap();
    let parent_id = parent.borrow().get_id();
    let child_id = child.borrow().get_id();

    manager.destroy_window(parent.clone()).unwrap();
    manager.update();

    assert_eq!(manager.window_count, 0);
    assert!(manager.get_window_by_id(parent_id).is_none());
    assert!(manager.get_window_by_id(child_id).is_none());
    assert!(
        parent
            .borrow()
            .get_status()
            .contains(WindowStatus::DESTROYED)
    );
    assert!(
        child
            .borrow()
            .get_status()
            .contains(WindowStatus::DESTROYED)
    );
}

#[test]
fn destroy_window_sends_parent_destroy_before_child_like_cpp() {
    let mut manager = WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager
        .create_window(Some(&parent), 10, 10, 20, 20)
        .unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));

    {
        let seen = seen.clone();
        parent
            .borrow_mut()
            .set_system_callback(move |_, msg, _, _| {
                if msg == WindowMessage::Destroy {
                    seen.borrow_mut().push("parent");
                }
                WindowMsgHandled::Ignored
            });
    }
    {
        let seen = seen.clone();
        child.borrow_mut().set_system_callback(move |_, msg, _, _| {
            if msg == WindowMessage::Destroy {
                seen.borrow_mut().push("child");
            }
            WindowMsgHandled::Ignored
        });
    }

    manager.destroy_window(parent).unwrap();
    manager.update();

    assert_eq!(seen.borrow().as_slice(), &["parent", "child"]);
}

#[test]
fn destroy_window_clears_runtime_references_like_cpp() {
    let mut manager = WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    window
        .borrow_mut()
        .set_system_callback(|_, msg, data1, data2| match msg {
            WindowMessage::InputFocus if data1 != 0 => {
                write_input_focus_response(data1, data2, true)
            }
            _ => WindowMsgHandled::Ignored,
        });

    manager.set_focus(Some(&window)).unwrap();
    manager.capture_mouse(&window).unwrap();
    manager.set_modal(window.clone()).unwrap();
    manager.current_mouse_region = Some(Rc::downgrade(&window));
    manager.set_grab_window(Some(&window));

    manager.destroy_window(window).unwrap();
    manager.update();

    assert!(manager.get_focus().is_none());
    assert!(manager.get_capture().is_none());
    assert!(manager.modal_stack.is_none());
    assert!(manager.current_mouse_region.is_none());
    assert!(manager.get_grab_window().is_none());
    assert!(!manager.capture_flags.contains(CaptureFlags::MOUSE));
}

#[test]
fn destroy_window_removes_window_from_layout_like_cpp() {
    let mut manager = WindowManager::new();
    let layout = Rc::new(RefCell::new(WindowLayout::new("test.wnd".to_string())));
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    window.borrow_mut().set_layout(Some(&layout));
    layout.borrow_mut().add_window(window.clone());

    manager.destroy_window(window.clone()).unwrap();
    manager.update();

    assert!(layout.borrow().windows().is_empty());
    assert!(window.borrow().get_layout().is_none());
}

#[test]
fn test_layout_hide_toggles_all_layout_windows_like_cpp() {
    let mut manager = WindowManager::new();
    let layout = manager.create_layout("test_layout.wnd".to_string());

    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager.create_window(Some(&parent), 5, 5, 20, 20).unwrap();
    child.borrow_mut().hide(true).unwrap();

    {
        let mut layout_mut = layout.borrow_mut();
        layout_mut.add_window(parent.clone());
        layout_mut.add_window(child.clone());
    }

    layout.borrow().hide(false);

    assert!(!parent.borrow().is_hidden());
    assert!(!child.borrow().is_hidden());
}

#[test]
fn test_tab_navigation() {
    let mut manager = WindowManager::new();
    let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let window2 = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let window3 = manager.create_window(None, 200, 0, 100, 100).unwrap();
    for window in [&window1, &window2, &window3] {
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
                _ => WindowMsgHandled::Ignored,
            });
    }

    manager.register_tab_list(vec![window1.clone(), window2.clone(), window3.clone()]);

    // Set initial focus
    manager.set_focus(Some(&window1)).unwrap();

    // Navigate forward
    manager.navigate_tab(TabDirection::Next);
    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window2, &focused));

    manager.navigate_tab(TabDirection::Next);
    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window3, &focused));

    // Should wrap around
    manager.navigate_tab(TabDirection::Next);
    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window1, &focused));

    // Navigate backward
    manager.navigate_tab(TabDirection::Previous);
    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window3, &focused));
}

#[test]
fn tab_navigation_is_blocked_by_modal_like_cpp() {
    let mut manager = WindowManager::new();
    let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let window2 = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let modal = manager.create_window(None, 0, 100, 100, 100).unwrap();
    for window in [&window1, &window2] {
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
                _ => WindowMsgHandled::Ignored,
            });
    }

    manager.register_tab_list(vec![window1.clone(), window2.clone()]);
    manager.set_focus(Some(&window1)).unwrap();
    manager.set_modal(modal).unwrap();

    manager.navigate_tab(TabDirection::Next);

    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window1, &focused));
}

#[test]
fn tab_navigation_clears_lone_window_like_cpp() {
    let mut manager = WindowManager::new();
    let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let window2 = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let lone = manager.create_window(None, 200, 0, 100, 100).unwrap();
    for window in [&window1, &window2] {
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
                _ => WindowMsgHandled::Ignored,
            });
    }
    let close_count = Rc::new(RefCell::new(0));
    {
        let close_count = Rc::clone(&close_count);
        lone.borrow_mut().set_system_callback(move |_, msg, _, _| {
            if msg == WindowMessage::User(16389) {
                *close_count.borrow_mut() += 1;
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
    }

    manager.register_tab_list(vec![window1.clone(), window2.clone()]);
    manager.set_focus(Some(&window1)).unwrap();
    manager.set_lone_window(Some(&lone));

    manager.navigate_tab(TabDirection::Next);

    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window2, &focused));
    assert!(manager.lone_window.is_none());
    assert_eq!(*close_count.borrow(), 1);
}

#[test]
fn prev_tab_without_tab_focus_wraps_to_last_like_cpp() {
    let mut manager = WindowManager::new();
    let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let window2 = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let outside_focus = manager.create_window(None, 200, 0, 100, 100).unwrap();
    for window in [&window1, &window2, &outside_focus] {
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
                _ => WindowMsgHandled::Ignored,
            });
    }

    manager.register_tab_list(vec![window1.clone(), window2.clone()]);
    manager.set_focus(Some(&outside_focus)).unwrap();

    manager.navigate_tab(TabDirection::Previous);

    let focused = manager.get_focus().unwrap();
    assert!(Rc::ptr_eq(&window2, &focused));
}

#[test]
fn test_window_layout() {
    let mut manager = WindowManager::new();
    let layout = manager.create_layout("test.wnd".to_string());

    let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let window2 = manager.create_window(None, 100, 100, 100, 100).unwrap();

    layout.borrow_mut().add_window(window1.clone());
    layout.borrow_mut().add_window(window2.clone());

    assert_eq!(layout.borrow().windows.len(), 2);
    assert_eq!(layout.borrow().get_filename(), "test.wnd");

    // Test hiding layout
    layout.borrow_mut().hide(true);
    assert!(layout.borrow().is_hidden());
    assert!(window1.borrow().is_hidden());
    assert!(window2.borrow().is_hidden());
}

#[test]
fn layout_add_window_pushes_to_head_like_cpp() {
    let mut manager = WindowManager::new();
    let mut layout = WindowLayout::new("test.wnd".to_string());
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 100, 0, 100, 100).unwrap();

    layout.add_window(first.clone());
    layout.add_window(second.clone());

    assert!(Rc::ptr_eq(&layout.windows()[0], &second));
    assert!(Rc::ptr_eq(&layout.windows()[1], &first));
}

#[test]
fn layout_add_and_remove_updates_neighbor_links_like_cpp() {
    let mut manager = WindowManager::new();
    let mut layout = WindowLayout::new("test.wnd".to_string());
    let first = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let second = manager.create_window(None, 100, 0, 100, 100).unwrap();
    let third = manager.create_window(None, 200, 0, 100, 100).unwrap();

    layout.add_window(first.clone());
    layout.add_window(second.clone());
    layout.add_window(third.clone());

    assert!(third.borrow().get_prev_in_layout().is_none());
    assert!(Rc::ptr_eq(
        &third.borrow().get_next_in_layout().unwrap(),
        &second
    ));
    assert!(Rc::ptr_eq(
        &second.borrow().get_prev_in_layout().unwrap(),
        &third
    ));
    assert!(Rc::ptr_eq(
        &second.borrow().get_next_in_layout().unwrap(),
        &first
    ));
    assert!(Rc::ptr_eq(
        &first.borrow().get_prev_in_layout().unwrap(),
        &second
    ));
    assert!(first.borrow().get_next_in_layout().is_none());

    layout.remove_window(&second);

    assert!(second.borrow().get_prev_in_layout().is_none());
    assert!(second.borrow().get_next_in_layout().is_none());
    assert!(Rc::ptr_eq(
        &third.borrow().get_next_in_layout().unwrap(),
        &first
    ));
    assert!(Rc::ptr_eq(
        &first.borrow().get_prev_in_layout().unwrap(),
        &third
    ));
}

#[test]
fn layout_add_and_remove_updates_window_back_reference_like_cpp() {
    let mut manager = WindowManager::new();
    let layout = manager.create_layout("test.wnd".to_string());
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

    layout.borrow_mut().add_window(window.clone());

    let window_layout = window.borrow().get_layout().unwrap();
    assert!(Rc::ptr_eq(&window_layout, &layout));
    drop(window_layout);

    layout.borrow_mut().remove_window(&window);

    assert!(window.borrow().get_layout().is_none());
    assert!(layout.borrow().windows().is_empty());
}

#[test]
fn reentrant_with_window_manager_does_not_panic() {
    let _lock = lock_test_mouse();
    with_window_manager(|manager| {
        manager.destroy_all_windows();
        with_window_manager(|manager| {
            manager.destroy_all_windows();
            with_window_manager(|_manager| ());
        });
    });
}

#[test]
fn reentrant_with_window_manager_unit_op_is_fail_closed_not_aliased() {
    let _lock = lock_test_mouse();
    let hits = Rc::new(Cell::new(0));
    let hits_inner = Rc::clone(&hits);
    with_window_manager(|manager| {
        manager.destroy_all_windows();
        with_window_manager(move |_manager| {
            hits_inner.set(hits_inner.get() + 1);
        });
        // Nested unit callback is dropped fail-closed, not transmuted into the queue.
        assert_eq!(hits.get(), 0);
    });
    assert_eq!(hits.get(), 0);

    let queued = Rc::new(Cell::new(0));
    let queued_inner = Rc::clone(&queued);
    with_window_manager(|_manager| {
        queue_window_manager_op(move |_manager| {
            queued_inner.set(queued_inner.get() + 1);
        });
        assert_eq!(queued.get(), 0);
    });
    assert_eq!(queued.get(), 1);
    with_window_manager(|manager| {
        manager.destroy_all_windows();
    });
}

#[test]
fn hide_parent_during_outstanding_borrow_applies_after_drain() {
    let _lock = lock_test_mouse();
    let parent = with_window_manager(|manager| {
        manager.destroy_all_windows();
        manager.update();
        manager.create_window(None, 0, 0, 100, 100).unwrap()
    });
    {
        let _held = parent.borrow_mut();
        with_window_manager(|_manager| {
            hide_window_rc(&parent, true);
        });
        // Outstanding `&mut` still held; hide is deferred, not applied.
    }
    with_window_manager(|_manager| {});
    assert!(
        parent.borrow().is_hidden(),
        "hide-parent must take effect after the outstanding borrow drains"
    );
    with_window_manager(|manager| {
        manager.destroy_all_windows();
        manager.update();
    });
}

#[test]
fn hide_and_enable_during_callback_do_not_panic() {
    let _lock = lock_test_mouse();
    let parent = with_window_manager(|manager| {
        manager.destroy_all_windows();
        manager.update();
        let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let _child = manager
            .create_window(Some(&parent), 10, 10, 20, 20)
            .unwrap();
        parent
    });
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_window_manager(|_manager| {
            parent.borrow_mut().enable(false).unwrap();
            parent.borrow_mut().hide(true).unwrap();
            parent.borrow_mut().enable(true).unwrap();
            parent.borrow_mut().hide(false).unwrap();
        });
    }));
    assert!(
        panicked.is_ok(),
        "enable/hide during with_window_manager must not panic"
    );
    assert!(!parent.borrow().is_hidden());
    with_window_manager(|manager| {
        manager.destroy_all_windows();
        manager.update();
    });
}

#[test]
fn nested_create_and_focus_queue_instead_of_fail_closed_noop() {
    let _lock = lock_test_mouse();
    let created = Rc::new(Cell::new(false));
    let created_flag = Rc::clone(&created);
    with_window_manager(|manager| {
        manager.destroy_all_windows();
        manager.update();
        let window = manager.create_window(None, 0, 0, 80, 24).unwrap();
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, data2| match msg {
                WindowMessage::InputFocus if data1 != 0 => {
                    write_input_focus_response(data1, data2, true)
                }
                _ => WindowMsgHandled::Ignored,
            });
        queue_set_focus(window);
        queue_create_layout("Menus/DoesNotNeedToExistForQueue.wnd");
        created_flag.set(true);
        assert_eq!(
            with_window_manager(|_m| true),
            false,
            "valued re-entry still fail-closes; create/focus must use queue"
        );
    });
    assert!(created.get());
    with_window_manager(|manager| {
        // Queued set_focus flushed when the outer borrow ended.
        assert!(manager.get_focus().is_some());
        manager.destroy_all_windows();
        manager.update();
    });
}

#[test]
fn reentrant_with_window_manager_value_is_fail_closed() {
    let _lock = lock_test_mouse();
    let result = with_window_manager(|manager| {
        manager.destroy_all_windows();
        with_window_manager(|_manager| true)
    });
    assert!(
        !result,
        "non-unit re-entry must fail-closed (bool default = false) without aliasing"
    );
    let found =
        with_window_manager(|manager| with_window_manager(|manager| manager.get_window_by_id(1)));
    assert!(found.is_none());
    with_window_manager(|manager| {
        manager.destroy_all_windows();
    });
}

#[test]
fn reentrant_with_window_manager_ref_is_fail_closed_without_alias() {
    let _lock = lock_test_mouse();
    with_window_manager(|manager| {
        manager.destroy_all_windows();
        let _ = manager.create_window(None, 0, 0, 10, 10).unwrap();
        let count = with_window_manager_ref(|manager| manager.root_window_count());
        assert_eq!(
            count, 0,
            "shared re-entry while mutably borrowed uses the empty fail-closed snapshot"
        );
        assert_eq!(manager.root_window_count(), 1);
        manager.destroy_all_windows();
    });
}

#[test]
fn layout_load_adds_only_top_level_windows_like_cpp() {
    let mut manager = WindowManager::new();
    let layout = manager.create_layout("test.wnd".to_string());
    let layout_def = WindowLayoutDefinition::default();
    let mut info = WindowLayoutInfo::default();
    let child = WindowDefinition {
        name: "ChildHidden".to_string(),
        window_type: "USER".to_string(),
        status: WindowStatus::HIDDEN,
        position: (10, 10),
        size: (20, 20),
        ..WindowDefinition::default()
    };
    let parent = WindowDefinition {
        name: "ParentRoot".to_string(),
        window_type: "USER".to_string(),
        position: (0, 0),
        size: (100, 100),
        children: vec![child],
        ..WindowDefinition::default()
    };

    manager
        .create_window_from_definition(&parent, None, &layout, &layout_def, &mut info)
        .expect("parent+child create");

    assert_eq!(
        info.windows.len(),
        1,
        "C++ scriptInfo.windows is roots only"
    );
    assert_eq!(layout.borrow().windows.len(), 1);
    layout.borrow().hide(false);
    let child_win = manager
        .find_window_by_name("ChildHidden")
        .expect("child exists");
    assert!(
        child_win.borrow().is_hidden(),
        "authored HIDDEN child must survive layout.hide(false)"
    );
}

#[test]
fn combo_field_click_opens_list_and_claims_lone_window() {
    let _lock = lock_test_mouse();
    let combo = with_window_manager(|manager| {
        manager.reset();
        let layout = Rc::new(RefCell::new(WindowLayout::new("test.wnd".to_string())));
        let layout_def = WindowLayoutDefinition::default();
        let mut info = WindowLayoutInfo::default();
        let combo_def = WindowDefinition {
            name: "test.wnd:Combo".to_string(),
            window_type: "COMBOBOX".to_string(),
            status: WindowStatus::ENABLED,
            style: GWS_COMBO_BOX,
            size: (120, 20),
            combo_box_data: Some(ComboBoxData {
                is_editable: false,
                max_display: 5,
                ..Default::default()
            }),
            ..WindowDefinition::default()
        };
        let combo = manager
            .create_window_from_definition(&combo_def, None, &layout, &layout_def, &mut info)
            .unwrap();
        let _ = with_payload(WindowMsgPayload::Text("Alpha".to_string()), |token| {
            combo
                .borrow_mut()
                .send_system_message(WindowMessage::User(GCM_ADD_ENTRY), token, 0)
        });
        let packed = 10usize | (10usize << 16);
        let _ = manager.process_mouse_event(WindowMessage::LeftUp, 10, 10, packed);
        combo
    });

    let links = combo.borrow().combobox_links().unwrap();
    let list = combo.borrow().find_child_by_id(links.list_box).unwrap();
    assert!(
        !list.borrow().is_hidden(),
        "combo field click must unhide the child list"
    );

    with_window_manager(|manager| {
        let lone = manager.lone_window.as_ref().and_then(|w| w.upgrade());
        assert!(
            lone.is_some_and(|window| Rc::ptr_eq(&window, &combo)),
            "opening a combo must claim the lone window"
        );
        manager.reset();
    });
}
