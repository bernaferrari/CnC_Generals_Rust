//! Hot key translator and manager.

use super::game_message::{GameMessage, GameMessageArgumentType, GameMessageType};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator};
use crate::game_text::GameText;
use crate::gui::game_window::{GameWindow, WindowMessage, WindowMsgData};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use log::warn;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::rc::Weak;

const KEY_STATE_CONTROL: u32 = 0x0004 | 0x0008;
const KEY_STATE_SHIFT: u32 = 0x0010 | 0x0020 | 0x0400;
const KEY_STATE_ALT: u32 = 0x0040 | 0x0080;

/// Session dedup for the duplicate-hotkey diagnostic.
///
/// C++ `HotKeyManager::addHotKey` (HotKey.cpp:123-137) DEBUG_ASSERTCRASHes on
/// a duplicate claim and silently ignores it in retail. The control bar
/// re-binds command windows repeatedly, so warn once per key per session and
/// demote repeats to debug instead of flooding the log every frame.
static DUPLICATE_HOTKEY_WARNED: LazyLock<std::sync::Mutex<HashSet<String>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashSet::new()));

/// True when this key already triggered its once-per-session duplicate warn.
pub(crate) fn duplicate_hotkey_warned(key: &str) -> bool {
    DUPLICATE_HOTKEY_WARNED
        .lock()
        .map(|warned| warned.contains(key))
        .unwrap_or(false)
}

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
            // C++ HotKey.cpp:130 DEBUG_ASSERTCRASH on duplicates, silent in
            // retail: the registration is ignored either way. Warn once per
            // key per session; repeats are debug-only.
            let should_warn = match DUPLICATE_HOTKEY_WARNED.lock() {
                Ok(mut warned) => warned.insert(key.clone()),
                Err(_) => false,
            };
            if should_warn {
                warn!(
                    "Hotkey {} already mapped; ignoring new window registration",
                    existing.key
                );
            } else {
                log::debug!(
                    "Hotkey {} already mapped; ignoring new window registration (repeat)",
                    existing.key
                );
            }
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

#[cfg(test)]
mod hot_key_dedup_tests {
    use super::*;

    #[test]
    fn duplicate_hotkey_registration_is_ignored_and_warns_once_per_session() {
        let key = "z";
        let first_window = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        let second_window = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        let first_id = first_window.borrow().get_id();

        let mut manager = HotKeyManager::default();
        manager.add_hot_key(first_window, key);
        // A second window claiming the same key is ignored (C++ HotKey.cpp:128).
        manager.add_hot_key(second_window, key);

        assert_eq!(
            manager.hot_key_map.len(),
            1,
            "only the first registration may own the key"
        );
        let owner_id = manager
            .hot_key_map
            .get(key)
            .and_then(|hot_key| hot_key.window.upgrade())
            .map(|window| window.borrow().get_id());
        assert_eq!(owner_id, Some(first_id), "the first window stays mapped");
        assert!(
            duplicate_hotkey_warned(key),
            "the duplicate claim must record its once-per-session warn"
        );
    }

    #[test]
    fn duplicate_warn_records_exactly_one_session_entry_per_key() {
        let key = "zz_dedup_probe";
        let mut manager = HotKeyManager::default();
        let window = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        manager.add_hot_key(window.clone(), key);
        for _ in 0..3 {
            manager.add_hot_key(window.clone(), key);
        }
        assert!(
            duplicate_hotkey_warned(key),
            "repeated duplicate claims stay recorded after the first warn"
        );
    }
}
