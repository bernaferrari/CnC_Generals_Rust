//! InGamePopupMessage.cpp callback port.

use crate::gui::control_bar::publish_host_dismiss_in_game_popup_message;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowMsgPayload,
    get_display_string_manager, get_font_library, payload, pop_payload, push_payload,
    queue_window_manager_op, with_window_manager, write_input_focus_response,
};
use crate::helpers::TheInGameUI;
use crate::message_stream::game_message::GameMessageType;
use crate::message_stream::message_stream::append_message_to_stream;
use game_engine::common::name_key_generator::NameKeyGenerator;
use log::warn;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const KEY_ENTER: usize = 0x0D;
const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

/// UI-local identity for a concrete popup layout instance. This is separate
/// from Main's host token: it lets a deferred Enter/Escape delivery reject an
/// old standalone popup after C++-style layout replacement, even though the
/// layout reuses the same numeric WND ids.
static NEXT_POPUP_UI_INSTANCE_GENERATION: AtomicUsize = AtomicUsize::new(1);

fn next_popup_ui_instance_generation() -> usize {
    loop {
        let generation = NEXT_POPUP_UI_INSTANCE_GENERATION.fetch_add(1, Ordering::Relaxed);
        if generation != 0 {
            return generation;
        }
    }
}

#[derive(Default)]
struct PopupUiState {
    parent_id: Option<u32>,
    static_text_id: Option<u32>,
    button_ok_id: Option<u32>,
    parent: Option<Rc<RefCell<GameWindow>>>,
    static_text: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
    pause: bool,
    /// Main's opaque identity for this exact popup instance. It is absent for
    /// standalone C++-compatibility callers, which retain their legacy route.
    host_popup_generation: Option<usize>,
    /// Per-layout identity used only to reject a queued keyboard delivery
    /// after a replacement popup takes over the same WND ids.
    popup_instance_generation: usize,
    /// Enter/Escape has queued a synthetic ButtonOk delivery but the outer
    /// WindowManager borrow has not drained it yet.
    dismissal_queued: bool,
    /// A ButtonOk acknowledgement has already been delivered for this active
    /// popup. Keep repeat key/mouse input from publishing duplicate host work
    /// before Main removes the popup WND.
    dismissal_published: bool,
}

thread_local! {
    static POPUP_UI_STATE: Arc<Mutex<PopupUiState>> =
        Arc::new(Mutex::new(PopupUiState::default()));
}

fn popup_ui_state() -> Arc<Mutex<PopupUiState>> {
    POPUP_UI_STATE.with(|state| state.clone())
}

pub fn in_game_popup_message_init(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let parent_id =
        NameKeyGenerator::name_to_key("InGamePopupMessage.wnd:InGamePopupMessageParent");
    let static_text_id = NameKeyGenerator::name_to_key("InGamePopupMessage.wnd:StaticTextMessage");
    let button_ok_id = NameKeyGenerator::name_to_key("InGamePopupMessage.wnd:ButtonOk");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let static_text = parent
        .as_ref()
        .and_then(|parent| parent.borrow().find_child_by_id(static_text_id as i32));
    let button_ok = parent
        .as_ref()
        .and_then(|parent| parent.borrow().find_child_by_id(button_ok_id as i32));

    let popup_data = TheInGameUI::get_popup_message_data();
    {
        let state_handle = popup_ui_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        state.parent_id = Some(parent_id);
        state.static_text_id = Some(static_text_id);
        state.button_ok_id = Some(button_ok_id);
        state.parent = parent.clone();
        state.static_text = static_text.clone();
        state.button_ok = button_ok.clone();
        state.pause = popup_data.as_ref().is_some_and(|data| data.pause);
        state.host_popup_generation = popup_data
            .as_ref()
            .and_then(|data| data.host_popup_generation);
        state.popup_instance_generation = next_popup_ui_instance_generation();
        state.dismissal_queued = false;
        state.dismissal_published = false;
    }

    let Some(popup_data) = popup_data else {
        warn!("InGamePopupMessageInit called without popup message data");
        return;
    };

    let message = popup_data.message.clone();

    let Some(static_text) = static_text else {
        return;
    };
    let Some(parent) = parent else {
        return;
    };
    let Some(button_ok) = button_ok else {
        return;
    };

    let text_height = {
        let mut display_manager = get_display_string_manager();
        let display_handle = display_manager.new_display_string();
        let mut display = display_handle.borrow_mut();
        display.set_text(message.clone());
        if let Some(font_desc) = static_text
            .borrow()
            .get_font()
            .map(|font| font.to_font_desc())
        {
            if let Ok(font_ref) = get_font_library().get_font(&font_desc) {
                display.set_font(font_ref);
            }
        }
        display.set_word_wrap(popup_data.width - 14);
        let (_, height) = display.get_size();
        drop(display);
        display_manager.free_display_string(display_handle);
        height
    };

    {
        let mut static_text_mut = static_text.borrow_mut();
        if let Some(widget) = static_text_mut.static_text_mut() {
            widget.set_text(message.clone());
        } else {
            let _ = static_text_mut.set_text(&message);
        }
        static_text_mut.set_enabled_text_colors(popup_data.text_color, 0);
    }

    let (button_width, button_height) = button_ok.borrow().get_size();

    let parent_height = text_height + 7 + 2 + 2 + button_height + 2;
    let _ = parent.borrow_mut().set_position(popup_data.x, popup_data.y);
    let _ = parent
        .borrow_mut()
        .set_size(popup_data.width, parent_height);

    let _ = static_text.borrow_mut().set_position(2, 2);
    let _ = static_text
        .borrow_mut()
        .set_size(popup_data.width - 4, text_height + 7);
    let _ = button_ok
        .borrow_mut()
        .set_position(popup_data.width - button_width - 2, text_height + 7 + 2 + 2);

    if popup_data.pause {
        with_window_manager(|manager| {
            let _ = manager.set_modal(parent.clone());
        });
    }

    with_window_manager(|manager| {
        let _ = manager.set_focus(Some(&parent));
    });
    let _ = parent.borrow_mut().hide(false);
    let _ = parent.borrow_mut().bring_to_front();
}

pub fn in_game_popup_message_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char {
        return WindowMsgHandled::Ignored;
    }

    let key = data1;
    let state = data2;
    if key != KEY_ENTER && key != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if (state & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    // `process_key_event` already holds WindowManager while it invokes this
    // callback. Snapshot and release our own state lock, then queue the
    // synchronous GadgetSelected delivery for that outer borrow's drain. A
    // nested `with_window_manager` would otherwise fail closed, while holding
    // this mutex across direct delivery would re-enter the popup system lock.
    let state_handle = popup_ui_state();
    let (window_id, button_ok_id, popup_instance_generation) = {
        let mut state_guard = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if state_guard.dismissal_queued || state_guard.dismissal_published {
            return WindowMsgHandled::Handled;
        }
        let Some(button_ok_id) = state_guard.button_ok_id else {
            return WindowMsgHandled::Handled;
        };
        state_guard.dismissal_queued = true;
        (
            window.get_id(),
            button_ok_id,
            state_guard.popup_instance_generation,
        )
    };

    queue_window_manager_op(move |manager| {
        // Always use a typed token, including for standalone popups.  A raw
        // zero is indistinguishable from an ordinary mouse selection and
        // could let a delayed legacy keyboard event A acknowledge a tagged
        // replacement popup B. The UI-instance generation distinguishes the
        // deferred keyboard path without repurposing C++'s raw message data.
        let instance_payload = push_payload(WindowMsgPayload::UInt(popup_instance_generation));
        if let Some(handle) = manager.get_window_by_id(window_id) {
            let _ = manager.send_system_message(
                &handle,
                WindowMessage::GadgetSelected,
                button_ok_id as WindowMsgData,
                instance_payload,
            );
        } else {
            // A layout can disappear between the keyboard callback and the
            // deferred delivery. Do not disturb a replacement popup's queued
            // acknowledgement when this one belongs to an older layout.
            let mut state_guard = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            if state_guard.popup_instance_generation == popup_instance_generation {
                state_guard.dismissal_queued = false;
            }
        }
        let _ = pop_payload(instance_payload);
    });

    WindowMsgHandled::Handled
}

pub fn in_game_popup_message_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        // The old layout's deferred destroy can run after C++-style popup
        // replacement has initialized a new layout with the same WND ids.
        // Init/reset and explicit data clear own PopupUiState lifecycle; this
        // callback must not mutate the global state based on an ambiguous WND.
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let queued_popup_instance_generation = match payload(data2) {
                Some(WindowMsgPayload::UInt(generation)) if generation != 0 => Some(generation),
                _ => None,
            };
            // Snapshot state before publishing or taking the standalone
            // fallback: both paths can synchronously revisit popup/window
            // state. The first delivery wins until the active WND is removed.
            let state_handle = popup_ui_state();
            let (pause, host_popup_generation) = {
                let mut state_guard = state_handle.lock().unwrap_or_else(|e| e.into_inner());
                if control_id != state_guard.button_ok_id.unwrap_or(0)
                    || state_guard.dismissal_published
                {
                    return WindowMsgHandled::Handled;
                }
                if queued_popup_instance_generation.is_some()
                    && queued_popup_instance_generation
                        != Some(state_guard.popup_instance_generation)
                {
                    // The deferred keyboard event belongs to a popup C++ has
                    // already replaced. Ignore it without consuming the new
                    // popup's future ButtonOk/Esc acknowledgement.
                    return WindowMsgHandled::Handled;
                }
                state_guard.dismissal_queued = false;
                state_guard.dismissal_published = true;
                (state_guard.pause, state_guard.host_popup_generation)
            };

            if let Some(host_popup_generation) = host_popup_generation {
                if publish_host_dismiss_in_game_popup_message(host_popup_generation) {
                    return WindowMsgHandled::Handled;
                }
            }

            if pause {
                TheInGameUI::clear_popup_message_data();
            } else {
                let _ = append_message_to_stream(GameMessageType::ClearInGamePopupMessage);
            }

            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::control_bar::{
        HostControlBarRequest, acquire_host_control_bar_bridge_test_guard,
        clear_host_control_bar_requests, set_host_control_bar_bridge_enabled,
        take_host_control_bar_requests,
    };
    use crate::gui::{WindowInputReturnCode, with_payload, with_window_manager};
    use crate::message_stream::get_message_stream;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn reset_popup_test_state() {
        let state_handle = popup_ui_state();
        *state_handle.lock().unwrap_or_else(|e| e.into_inner()) = PopupUiState::default();
        with_window_manager(|manager| manager.reset());
    }

    struct PopupTestReset;

    impl Drop for PopupTestReset {
        fn drop(&mut self) {
            reset_popup_test_state();
            let stream = get_message_stream();
            stream
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .clear_messages();
        }
    }

    fn set_popup_button_ok_id(
        button_ok_id: u32,
        pause: bool,
        host_popup_generation: Option<usize>,
    ) {
        let state_handle = popup_ui_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        state.button_ok_id = Some(button_ok_id);
        state.pause = pause;
        state.host_popup_generation = host_popup_generation.filter(|generation| *generation != 0);
        state.popup_instance_generation = next_popup_ui_instance_generation();
        state.dismissal_queued = false;
        state.dismissal_published = false;
    }

    #[test]
    fn popup_system_consumes_lifecycle_messages_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            in_game_popup_message_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            in_game_popup_message_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            in_game_popup_message_system(&window, WindowMessage::InputFocus, 1, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            in_game_popup_message_system(&window, WindowMessage::MousePos, 0, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn popup_keyboard_acknowledgement_releases_popup_mutex_before_delivery() {
        let _reset = PopupTestReset;
        reset_popup_test_state();
        const WINDOW_ID: i32 = 913_701;
        const BUTTON_OK_ID: u32 = 913_702;
        let popup_mutex_was_available = Arc::new(AtomicBool::new(false));

        with_window_manager(|manager| {
            let target = manager
                .create_window_with_id(None, 0, 0, 1, 1, WINDOW_ID)
                .expect("synthetic popup target");
            let popup_mutex_was_available = Arc::clone(&popup_mutex_was_available);
            target
                .borrow_mut()
                .set_system_callback(move |_, msg, _, _| {
                    if msg == WindowMessage::GadgetSelected {
                        popup_mutex_was_available
                            .store(popup_ui_state().try_lock().is_ok(), Ordering::SeqCst);
                    }
                    WindowMsgHandled::Handled
                });
        });

        let mut dispatch_window = GameWindow::new();
        dispatch_window.set_id(WINDOW_ID);
        for key in [KEY_ENTER, KEY_ESC] {
            set_popup_button_ok_id(BUTTON_OK_ID, false, None);
            popup_mutex_was_available.store(false, Ordering::SeqCst);
            assert_eq!(
                in_game_popup_message_input(
                    &dispatch_window,
                    WindowMessage::Char,
                    key as WindowMsgData,
                    KEY_STATE_UP as WindowMsgData,
                ),
                WindowMsgHandled::Handled
            );
            assert!(
                popup_mutex_was_available.load(Ordering::SeqCst),
                "{key:#x} delivery must not re-enter the popup mutex"
            );
            let state_handle = popup_ui_state();
            *state_handle.lock().unwrap_or_else(|e| e.into_inner()) = PopupUiState::default();
        }
    }

    #[test]
    fn popup_keyboard_acknowledgement_queues_one_host_request_after_input_dispatch() {
        let _bridge_guard = acquire_host_control_bar_bridge_test_guard();
        let _reset = PopupTestReset;
        reset_popup_test_state();
        set_host_control_bar_bridge_enabled(true);
        const WINDOW_ID: i32 = 913_711;
        const BUTTON_OK_ID: u32 = 913_712;
        const HOST_POPUP_GENERATION: usize = 913_713;

        for key in [KEY_ENTER, KEY_ESC] {
            clear_host_control_bar_requests();
            set_popup_button_ok_id(BUTTON_OK_ID, false, Some(HOST_POPUP_GENERATION));
            with_window_manager(|manager| {
                manager.reset();
                let popup = manager
                    .create_window_with_id(None, 0, 0, 1, 1, WINDOW_ID)
                    .expect("synthetic popup window");
                popup
                    .borrow_mut()
                    .set_input_callback(in_game_popup_message_input);
                popup
                    .borrow_mut()
                    .set_system_callback(in_game_popup_message_system);
                manager
                    .set_focus(Some(&popup))
                    .expect("focus synthetic popup");

                assert_eq!(
                    manager.process_key_event(key as u8, KEY_STATE_UP as u8),
                    WindowInputReturnCode::Used
                );
                assert_eq!(
                    manager.process_key_event(key as u8, KEY_STATE_UP as u8),
                    WindowInputReturnCode::Used,
                    "repeat key before deferred delivery must not publish a duplicate ACK"
                );
                assert!(
                    take_host_control_bar_requests().is_empty(),
                    "delivery stays deferred until the outer manager borrow drains"
                );
            });
            assert!(matches!(
                take_host_control_bar_requests().as_slice(),
                [HostControlBarRequest::DismissInGamePopupMessage {
                    popup_generation: HOST_POPUP_GENERATION
                }]
            ));
            let state_handle = popup_ui_state();
            *state_handle.lock().unwrap_or_else(|e| e.into_inner()) = PopupUiState::default();
        }
    }

    #[test]
    fn popup_acknowledgement_preserves_disabled_legacy_routes() {
        let _bridge_guard = acquire_host_control_bar_bridge_test_guard();
        let _reset = PopupTestReset;
        reset_popup_test_state();
        const BUTTON_OK_ID: u32 = 913_721;
        let window = GameWindow::new();
        let stream = get_message_stream();

        for pause in [false, true] {
            stream
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .clear_messages();
            set_popup_button_ok_id(BUTTON_OK_ID, pause, None);
            assert_eq!(
                in_game_popup_message_system(
                    &window,
                    WindowMessage::GadgetSelected,
                    BUTTON_OK_ID as WindowMsgData,
                    0,
                ),
                WindowMsgHandled::Handled
            );
            assert!(take_host_control_bar_requests().is_empty());
            let stream_guard = stream.read().unwrap_or_else(|e| e.into_inner());
            assert_eq!(
                stream_guard.contains_message_of_type(&GameMessageType::ClearInGamePopupMessage),
                !pause,
                "non-pausing popups retain C++ message-stream clear; pausing popups retain direct clear"
            );
            drop(stream_guard);
            let state_handle = popup_ui_state();
            *state_handle.lock().unwrap_or_else(|e| e.into_inner()) = PopupUiState::default();
        }
    }

    #[test]
    fn popup_keyboard_ack_for_replaced_layout_is_ignored_without_consuming_replacement() {
        let _bridge_guard = acquire_host_control_bar_bridge_test_guard();
        let _reset = PopupTestReset;
        reset_popup_test_state();
        set_host_control_bar_bridge_enabled(true);
        const BUTTON_OK_ID: u32 = 913_731;
        const ACTIVE_HOST_GENERATION: usize = 913_732;

        // This state represents replacement popup B.  Its own current
        // per-layout identity is intentionally different from old popup A's
        // deferred keyboard delivery below.
        set_popup_button_ok_id(BUTTON_OK_ID, false, Some(ACTIVE_HOST_GENERATION));
        let state_handle = popup_ui_state();
        let active_instance_generation = state_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .popup_instance_generation;
        let stale_instance_generation = if active_instance_generation == 1 {
            2
        } else {
            1
        };
        let window = GameWindow::new();

        with_payload(
            WindowMsgPayload::UInt(stale_instance_generation),
            |payload| {
                assert_eq!(
                    in_game_popup_message_system(
                        &window,
                        WindowMessage::GadgetSelected,
                        BUTTON_OK_ID as WindowMsgData,
                        payload,
                    ),
                    WindowMsgHandled::Handled
                );
            },
        );

        assert!(
            take_host_control_bar_requests().is_empty(),
            "delayed A must not emit an ACK for active replacement B"
        );
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(state.host_popup_generation, Some(ACTIVE_HOST_GENERATION));
        assert!(!state.dismissal_published);
    }

    #[test]
    fn popup_destroy_does_not_clear_a_replacement_identity() {
        let _reset = PopupTestReset;
        reset_popup_test_state();
        const BUTTON_OK_ID: u32 = 913_741;
        const HOST_POPUP_GENERATION: usize = 913_742;
        set_popup_button_ok_id(BUTTON_OK_ID, false, Some(HOST_POPUP_GENERATION));
        let window = GameWindow::new();

        assert_eq!(
            in_game_popup_message_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            popup_ui_state()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .host_popup_generation,
            Some(HOST_POPUP_GENERATION),
            "Destroy has no safe per-layout identity and must not erase replacement B"
        );
    }
}
