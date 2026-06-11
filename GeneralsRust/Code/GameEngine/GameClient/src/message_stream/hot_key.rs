//! Hot key translator and manager.

use super::game_message::{GameMessage, GameMessageArgumentType, GameMessageType};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator};
use crate::game_text::GameText;
use crate::gui::game_window::{GameWindow, WindowMessage, WindowMsgData};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use log::warn;
use std::collections::HashMap;
use std::rc::Weak;

const KEY_STATE_CONTROL: u32 = 0x0004 | 0x0008;
const KEY_STATE_SHIFT: u32 = 0x0010 | 0x0020 | 0x0400;
const KEY_STATE_ALT: u32 = 0x0040 | 0x0080;

fn keycode_to_char(key_code: u32) -> Option<char> {
    match key_code {
        0x30..=0x39 => char::from_u32(key_code),
        0x41..=0x5A => char::from_u32(key_code),
        0x61..=0x7A => char::from_u32(key_code),
        _ => None,
    }
}

fn extract_key_state(msg: &GameMessage) -> u32 {
    match msg.get_argument(1) {
        Some(GameMessageArgumentType::Integer(value)) => *value as u32,
        _ => 0,
    }
}

#[derive(Default)]
pub struct HotKeyTranslator;

impl HotKeyTranslator {
    pub fn new() -> Self {
        Self
    }
}

impl GameMessageTranslator for HotKeyTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let GameMessageType::RawKeyUp(key_code) = msg.get_type() else {
            return GameMessageDisposition::KeepMessage;
        };

        let key_state = extract_key_state(msg);
        let mut modifier_state = 0u32;
        if (key_state & KEY_STATE_CONTROL) != 0 {
            modifier_state |= KEY_STATE_CONTROL;
        }
        if (key_state & KEY_STATE_SHIFT) != 0 {
            modifier_state |= KEY_STATE_SHIFT;
        }
        if (key_state & KEY_STATE_ALT) != 0 {
            modifier_state |= KEY_STATE_ALT;
        }
        if modifier_state != 0 {
            return GameMessageDisposition::KeepMessage;
        }

        let Some(key_char) = keycode_to_char(*key_code) else {
            return GameMessageDisposition::KeepMessage;
        };
        let mut key_string = key_char.to_string();
        key_string.make_ascii_lowercase();

        if with_hot_key_manager(|manager| manager.execute_hot_key(&key_string)) {
            GameMessageDisposition::DestroyMessage
        } else {
            GameMessageDisposition::KeepMessage
        }
    }
}

#[derive(Default)]
pub struct HotKey {
    key: String,
    window: Weak<std::cell::RefCell<GameWindow>>,
}

#[derive(Default)]
pub struct HotKeyManager {
    hot_key_map: HashMap<String, HotKey>,
}

impl HotKeyManager {
    pub fn init(&mut self) {
        self.hot_key_map.clear();
    }

    pub fn reset(&mut self) {
        self.hot_key_map.clear();
    }

    pub fn add_hot_key(
        &mut self,
        window: std::rc::Rc<std::cell::RefCell<GameWindow>>,
        key_in: &str,
    ) {
        let mut key = key_in.to_ascii_lowercase();
        if key.is_empty() {
            return;
        }

        if let Some(existing) = self.hot_key_map.get(&key) {
            warn!(
                "Hotkey {} already mapped; ignoring new window registration",
                existing.key
            );
            return;
        }

        let hot_key = HotKey {
            key: key.clone(),
            window: std::rc::Rc::downgrade(&window),
        };
        self.hot_key_map.insert(key, hot_key);
    }

    pub fn execute_hot_key(&mut self, key_in: &str) -> bool {
        let key = key_in.to_ascii_lowercase();
        let Some(entry) = self.hot_key_map.get(&key) else {
            return false;
        };
        let Some(window_rc) = entry.window.upgrade() else {
            return false;
        };

        let (window_id, parent) = {
            let window = window_rc.borrow();
            (window.get_id(), window.get_parent())
        };

        {
            let window = window_rc.borrow();
            if window.is_hidden() {
                return false;
            }
            if window.is_enabled() {
                drop(window);
                if let Some(parent_rc) = parent {
                    parent_rc.borrow_mut().send_system_message(
                        WindowMessage::GadgetSelected,
                        window_id as WindowMsgData,
                        0,
                    );
                } else {
                    window_rc.borrow_mut().send_system_message(
                        WindowMessage::GadgetSelected,
                        window_id as WindowMsgData,
                        0,
                    );
                }

                if let Some(audio) = TheAudio::get() {
                    let event = AudioEventRts::with_event_name("GUIClick");
                    audio.add_audio_event(&event);
                }

                return true;
            }
        }

        if let Some(audio) = TheAudio::get() {
            let event = AudioEventRts::with_event_name("GUIClickDisabled");
            audio.add_audio_event(&event);
        }
        false
    }

    pub fn search_hot_key(&self, label: &str) -> String {
        let localized = GameText::fetch(label);
        self.search_hot_key_in_text(&localized)
    }

    pub fn search_hot_key_in_text(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '&' {
                if let Some(next) = chars.peek() {
                    return next.to_string();
                }
            }
        }
        String::new()
    }
}

pub fn with_hot_key_manager<R>(f: impl FnOnce(&mut HotKeyManager) -> R) -> R {
    thread_local! {
        static HOT_KEY_MANAGER: std::cell::RefCell<HotKeyManager> =
            std::cell::RefCell::new(HotKeyManager::default());
    }
    HOT_KEY_MANAGER.with(|manager| f(&mut manager.borrow_mut()))
}
