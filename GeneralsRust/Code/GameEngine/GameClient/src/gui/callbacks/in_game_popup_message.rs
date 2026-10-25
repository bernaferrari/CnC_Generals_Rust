//! InGamePopupMessage.cpp: one receiving client's popup and callback phases.
use crate::gui::game_window::WindowEventEffects;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowMsgPayload,
    payload, pop_payload, push_payload, write_input_focus_response,
};
use crate::helpers::PopupMessageData;
use crate::message_stream::game_message::{GameMessage, GameMessageType};
use game_engine::common::name_key_generator::NameKeyGenerator;

const KEY_ENTER: usize = 0x0d;
const KEY_ESC: usize = 0x1b;
const KEY_STATE_UP: usize = 1;
#[derive(Default)]
pub struct PopupMessageState {
    data: Option<PopupMessageData>,
    parent: Option<std::rc::Weak<std::cell::RefCell<GameWindow>>>,
    button_ok_id: Option<u32>,
    generation: usize,
    next_generation: usize,
    dismissal_published: bool,
}
impl PopupMessageState {
    fn begin_instance(&mut self) {
        self.next_generation = self.next_generation.wrapping_add(1);
        if self.next_generation == 0 {
            self.next_generation = 1;
        }
        self.generation = self.next_generation;
        self.button_ok_id = Some(NameKeyGenerator::name_to_key(
            "InGamePopupMessage.wnd:ButtonOk",
        ));
        self.dismissal_published = false;
        self.parent = None;
    }
    fn accept_selection(&mut self, id: u32, token: Option<usize>) -> Option<(bool, Option<usize>)> {
        if self.button_ok_id != Some(id)
            || self.dismissal_published
            || token.is_some_and(|token| token != self.generation)
        {
            return None;
        }
        let data = self.data.as_ref()?;
        self.dismissal_published = true;
        Some((data.pause, data.host_popup_generation))
    }
}
impl WindowEventEffects<'_> {
    pub(crate) fn with_popup_message<R>(
        &mut self,
        operation: impl FnOnce(&mut PopupMessageState, &mut WindowEventEffects<'_>) -> R,
    ) -> R {
        let state = self
            .menus
            .as_mut()
            .expect("popup requires receiving client menus")
            .popup
            .take()
            .expect("same-popup reentry must follow the current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(state, self)));
        self.menus
            .as_mut()
            .expect("popup menus remain installed")
            .popup = Some(state);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

/// C++ clears the prior popup before allocating/initializing its replacement.
/// The owned birth continuation ends at the enclosing callback/action boundary;
/// no frame-late reconciliation or global window selection is involved.
pub fn show_receiving_popup_message(
    identifier: &str,
    x: i32,
    y: i32,
    width: i32,
    text_color: u32,
    pause: bool,
    pause_music: bool,
    host_popup_generation: Option<usize>,
    effects: &mut WindowEventEffects<'_>,
) {
    let identifier = identifier.to_owned();
    effects.queue_window_operation(move |manager, effects| {
        let previous = effects.with_popup_message(|state, _| state.data.take());
        if let Some(previous) = previous {
            if let Some(layout) = previous.layout {
                manager.destroy_layout(&layout, effects);
            }
        }
        crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text(
            &identifier,
            false,
            manager,
        );
        let (screen_w, screen_h) = manager.screen_size();
        let data = PopupMessageData {
            message: crate::game_text::GameText::fetch(&identifier),
            x: (screen_w as f32 * (x.clamp(0, 100) as f32 / 100.0)) as i32,
            y: (screen_h as f32 * (y.clamp(0, 100) as f32 / 100.0)) as i32,
            width: width.max(50),
            text_color,
            pause,
            pause_music,
            host_popup_generation,
            layout: None,
        };
        effects.with_popup_message(|state, _| {
            state.data = Some(data);
            state.begin_instance();
        });
        let Ok((layout, _)) = manager.create_layout_with_windows("InGamePopupMessage.wnd", effects)
        else {
            return;
        };
        effects.with_popup_message(|state, _| {
            if let Some(data) = state.data.as_mut() {
                data.layout = Some(layout.clone());
            }
        });
        effects.with_receiving_manager(manager, |effects| {
            layout.borrow().run_init_with_effects(None, effects)
        });
    });
}

/// Remove only this client's receipt. Physical destruction follows the bounded
/// current-window callback loan; Destroy callbacks themselves do not read data
/// (InGamePopupMessage.cpp System GWM_DESTROY is inert).
pub fn clear_receiving_popup_message(
    effects: &mut WindowEventEffects<'_>,
) -> Option<PopupMessageData> {
    let data = effects.with_popup_message(|state, _| {
        state.parent = None;
        state.button_ok_id = None;
        state.dismissal_published = false;
        state.data.take()
    });
    if let Some(layout) = data.as_ref().and_then(|data| data.layout.clone()) {
        effects.queue_window_operation(move |manager, effects| {
            manager.destroy_layout(&layout, effects)
        });
    }
    data
}
pub fn receiving_popup_message(effects: &mut WindowEventEffects<'_>) -> Option<PopupMessageData> {
    effects.with_popup_message(|state, _| state.data.clone())
}

pub fn in_game_popup_message_init(
    _layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    effects: &mut WindowEventEffects<'_>,
) {
    let Some(data) = receiving_popup_message(effects) else {
        return;
    };
    effects.with_manager(|manager, effects| {
        let parent_id =
            NameKeyGenerator::name_to_key("InGamePopupMessage.wnd:InGamePopupMessageParent") as i32;
        let Some(parent) = manager.get_window_by_id(parent_id) else {
            return;
        };
        effects.with_popup_message(|state, _| state.parent = Some(std::rc::Rc::downgrade(&parent)));
        let static_id =
            NameKeyGenerator::name_to_key("InGamePopupMessage.wnd:StaticTextMessage") as i32;
        let button_id = NameKeyGenerator::name_to_key("InGamePopupMessage.wnd:ButtonOk") as i32;
        let Some(text) = parent.borrow().find_child_by_id(static_id) else {
            return;
        };
        let Some(button) = parent.borrow().find_child_by_id(button_id) else {
            return;
        };
        let mut display = crate::gui::display_string::DisplayString::new();
        display.set_text(data.message.clone());
        if let Some(font) = text
            .borrow()
            .get_font()
            .map(|font| font.to_font_desc())
            .and_then(|font| crate::gui::get_font_library().get_font(&font).ok())
        {
            display.set_font(font);
        }
        display.set_word_wrap(data.width - 14);
        let (_, text_height) = display.get_size();
        {
            let mut text = text.borrow_mut();
            if let Some(widget) = text.static_text_mut() {
                widget.set_text(data.message.clone());
            } else {
                let _ = text.set_text(&data.message);
            }
        }
        let (button_w, button_h) = button.borrow().get_size();
        let _ = parent.borrow_mut().set_position(data.x, data.y);
        effects.with_receiving_manager(manager, |effects| {
            let _ = parent.borrow_mut().set_size(
                data.width,
                text_height + 7 + 2 + 2 + button_h + 2,
                effects,
            );
        });
        let _ = text.borrow_mut().set_position(2, 2);
        effects.with_receiving_manager(manager, |effects| {
            let _ = text
                .borrow_mut()
                .set_size(data.width - 4, text_height + 7, effects);
        });
        let _ = button
            .borrow_mut()
            .set_position(data.width - button_w - 2, text_height + 7 + 2 + 2);
        text.borrow_mut()
            .set_enabled_text_colors(data.text_color, 0);
        if data.pause {
            let _ = manager.set_modal(parent.clone());
        }
        let _ = manager.set_focus(Some(&parent), effects);
        let _ = manager.hide_window(&parent, false);
        let _ = parent.borrow_mut().bring_to_front();
    });
}

pub fn in_game_popup_message_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || (data1 != KEY_ENTER && data1 != KEY_ESC) {
        return WindowMsgHandled::Ignored;
    }
    if data2 & KEY_STATE_UP == 0 {
        return WindowMsgHandled::Handled;
    }
    let Some((button, generation)) = effects
        .with_popup_message(|state, _| state.button_ok_id.map(|button| (button, state.generation)))
    else {
        return WindowMsgHandled::Handled;
    };
    let token = push_payload(effects, WindowMsgPayload::UInt(generation));
    // C++ sends GBM_SELECTED to this exact canonical window synchronously.
    let _ = window.send_system_message(
        WindowMessage::GadgetSelected,
        button as WindowMsgData,
        token,
        effects,
    );
    let _ = pop_payload(effects, token);
    WindowMsgHandled::Handled
}
pub fn in_game_popup_message_system_with_sink(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy | WindowMessage::None => {
            WindowMsgHandled::Handled
        }
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let token = match payload(effects, data2) {
                Some(WindowMsgPayload::UInt(value)) if value != 0 => Some(value),
                _ => None,
            };
            let Some((pause, generation)) = effects.with_popup_message(|state, _| {
                // Reused WND ids cannot authorize an old layout's callback.
                let parent = state.parent.as_ref()?.upgrade()?;
                if !std::ptr::eq(parent.as_ref().as_ptr(), window as *mut GameWindow) {
                    return None;
                }
                state.accept_selection(data1 as u32, token)
            }) else {
                return WindowMsgHandled::Handled;
            };
            if pause {
                clear_receiving_popup_message(effects);
            }
            if let Some(generation) = generation {
                effects.menu_host.as_deref_mut().expect("native popup requires its receiving application mailbox").request(crate::gui::campaign_launch_host_bridge::MenuHostRequest::AcknowledgePopup{generation});
            } else if !pause {
                effects
                    .emitted_messages
                    .push(GameMessage::new(GameMessageType::ClearInGamePopupMessage));
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active(generation: usize) -> PopupMessageState {
        let mut state = PopupMessageState::default();
        state.data = Some(PopupMessageData {
            message: "message".into(),
            x: 0,
            y: 0,
            width: 50,
            text_color: 0xffffffff,
            pause: false,
            pause_music: false,
            host_popup_generation: Some(generation),
            layout: None,
        });
        state.begin_instance();
        state
    }
    #[test]
    fn repeated_acknowledgement_is_published_once() {
        let mut state = active(9);
        let button = state.button_ok_id.unwrap();
        assert_eq!(
            state.accept_selection(button, Some(state.generation)),
            Some((false, Some(9)))
        );
        assert_eq!(state.accept_selection(button, None), None);
    }
    #[test]
    fn stale_keyboard_token_does_not_consume_replacement() {
        let mut state = active(1);
        let old = state.generation;
        state.begin_instance();
        let button = state.button_ok_id.unwrap();
        assert_eq!(state.accept_selection(button, Some(old)), None);
        assert_eq!(
            state.accept_selection(button, Some(state.generation)),
            Some((false, Some(1)))
        );
    }
    #[test]
    fn clients_own_independent_popup_generations() {
        let mut a = active(1);
        let b = active(2);
        a.begin_instance();
        assert_ne!(a.generation, b.generation);
        assert_eq!(b.data.as_ref().unwrap().host_popup_generation, Some(2));
        assert!(!b.dismissal_published);
    }
}
