//! InGamePopupMessage.cpp callback port.

use crate::gui::{
    get_display_string_manager, get_font_library, with_window_manager, GameWindow, WindowLayout,
    WindowMessage, WindowMsgData, WindowMsgHandled,
};
use crate::helpers::TheInGameUI;
use crate::message_stream::game_message::GameMessageType;
use crate::message_stream::message_stream::append_message_to_stream;
use game_engine::common::name_key_generator::NameKeyGenerator;
use log::warn;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ENTER: u32 = 0x0D;
const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

#[derive(Default)]
struct PopupUiState {
    parent_id: Option<u32>,
    static_text_id: Option<u32>,
    button_ok_id: Option<u32>,
    parent: Option<Rc<RefCell<GameWindow>>>,
    static_text: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
    pause: bool,
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

    let state_handle = popup_ui_state();
    let mut state = state_handle.lock().expect("popup ui state lock poisoned");
    state.parent_id = Some(parent_id);
    state.static_text_id = Some(static_text_id);
    state.button_ok_id = Some(button_ok_id);
    state.parent = parent.clone();
    state.static_text = static_text.clone();
    state.button_ok = button_ok.clone();

    let Some(popup_data) = TheInGameUI::get_popup_message_data() else {
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

    state.pause = popup_data.pause;

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

    let state_handle = popup_ui_state();
    let state_guard = state_handle.lock().expect("popup ui state lock poisoned");
    let button_ok_id = state_guard.button_ok_id.unwrap_or(0) as u32;

    with_window_manager(|manager| {
        if let Some(handle) = manager.get_window_by_id(window.get_id()) {
            manager.send_system_message(
                &handle,
                WindowMessage::GadgetSelected,
                button_ok_id,
                button_ok_id,
            );
        }
    });

    WindowMsgHandled::Handled
}

pub fn in_game_popup_message_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let state_handle = popup_ui_state();
            let state_guard = state_handle.lock().expect("popup ui state lock poisoned");
            let button_ok_id = state_guard.button_ok_id.unwrap_or(0);

            if control_id == button_ok_id {
                if state_guard.pause {
                    TheInGameUI::clear_popup_message_data();
                } else {
                    let _ = append_message_to_stream(GameMessageType::ClearInGamePopupMessage);
                }
            }

            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
