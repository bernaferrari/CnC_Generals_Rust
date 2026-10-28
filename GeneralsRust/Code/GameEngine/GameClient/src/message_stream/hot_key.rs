//! Hot key translator and manager.

use super::game_message::{GameMessage, GameMessageArgumentType, GameMessageType};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator};
use crate::game_text::GameText;
use crate::gui::game_window::{GameWindow, WindowMessage, WindowMsgData};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use log::warn;
use std::collections::{HashMap, HashSet};
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
    fn requires_receiving_window_context(&self) -> bool {
        true
    }
    fn translate_game_message(&mut self, _msg: &GameMessage) -> GameMessageDisposition {
        log::error!("Hotkey translation requires receiving UI context");
        GameMessageDisposition::KeepMessage
    }
    fn translate_game_message_with_context(
        &mut self,
        msg: &GameMessage,
        client: game_engine::common::message_stream::TranslatorClientObservation,
        dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,
        emitted: &mut Vec<GameMessage>,
        pick_view: Option<&game_engine::common::message_stream::TranslatorPickObjectView>,
        client_ui: &mut dyn std::any::Any,
        drawable_owner: Option<&mut dyn std::any::Any>,
    ) -> GameMessageDisposition {
        let ui = &mut crate::message_stream::ReceivingUi::new_with_owner(
            client_ui,
            drawable_owner,
            pick_view,
            client.logic_frame,
            client.frame,
        )
        .with_emitter(emitted);

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
        if match ui.dispatch(
            dispatch,
            game_engine::common::message_stream::WindowInputDispatch::Ui(
                game_engine::common::message_stream::UiInputAction::ActivateHotKey(key_char as u32),
            ),
        ) {
            Some(game_engine::common::message_stream::WindowInputDispatchResult::Ui(handled)) => {
                handled
            }
            _ => panic!("hotkey operation requires its receiving UI driver"),
        } {
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
    // Rust diagnostic only; C++ addHotKey ignores duplicate registrations.
    // Dedup belongs to the same manager session as the map it observes.
    duplicate_warnings: HashSet<String>,
}

impl HotKeyManager {
    pub fn init(&mut self) {
        self.hot_key_map.clear();
        self.duplicate_warnings.clear();
    }

    pub fn reset(&mut self) {
        self.hot_key_map.clear();
        self.duplicate_warnings.clear();
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
            let should_warn = self.duplicate_warnings.insert(key.clone());
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

    pub(crate) fn activation(&self, key_in: &str) -> Option<HotKeyActivation> {
        let entry = self.hot_key_map.get(&key_in.to_ascii_lowercase())?;
        let window = entry.window.upgrade()?;
        let (id, enabled, parent) = {
            let value = window.borrow();
            if value.is_hidden() {
                return None;
            }
            (value.get_id(), value.is_enabled(), value.get_parent())
        };
        Some(HotKeyActivation {
            window,
            parent,
            id,
            enabled,
        })
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

pub(crate) struct HotKeyActivation {
    window: std::rc::Rc<std::cell::RefCell<GameWindow>>,
    parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
    id: i32,
    enabled: bool,
}
impl HotKeyActivation {
    pub(crate) fn execute(
        self,
        manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> bool {
        if !manager.owns_window_identity(&self.window) {
            return false;
        }
        if self.enabled {
            let target = self.parent.unwrap_or(self.window);
            manager.send_system_message(
                &target,
                WindowMessage::GadgetSelected,
                self.id as WindowMsgData,
                0,
                effects,
            );
        }
        if let Some(audio) = TheAudio::get() {
            audio.add_audio_event(&AudioEventRts::with_event_name(if self.enabled {
                "GUIClick"
            } else {
                "GUIClickDisabled"
            }));
        }
        self.enabled
    }
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
        manager.add_hot_key(first_window.clone(), key);
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
            manager.duplicate_warnings.contains(key),
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
            manager.duplicate_warnings.contains(key),
            "repeated duplicate claims stay recorded after the first warn"
        );
    }
    #[test]
    fn duplicate_warnings_follow_the_receiving_manager_lifetime() {
        let window = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        let other = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        let mut first = HotKeyManager::default();
        first.add_hot_key(window.clone(), "Q");
        first.add_hot_key(other.clone(), "q");
        assert_eq!(first.duplicate_warnings.len(), 1);
        assert!(first.duplicate_warnings.contains("q"));

        let mut peer = HotKeyManager::default();
        assert!(peer.duplicate_warnings.is_empty());
        peer.add_hot_key(other.clone(), "Q");
        assert!(peer.duplicate_warnings.is_empty());
        peer.add_hot_key(window.clone(), "q");
        assert_eq!(peer.duplicate_warnings.len(), 1);
        assert_eq!(peer.hot_key_map.len(), 1);

        peer.init();
        assert!(peer.hot_key_map.is_empty());
        assert!(peer.duplicate_warnings.is_empty());
        assert_eq!(first.hot_key_map.len(), 1);
        assert!(first.duplicate_warnings.contains("q"));

        peer.add_hot_key(other.clone(), "Q");
        peer.add_hot_key(window.clone(), "q");
        peer.reset();
        assert!(peer.hot_key_map.is_empty());
        assert!(peer.duplicate_warnings.is_empty());
        assert_eq!(first.hot_key_map.len(), 1);
        assert!(first.duplicate_warnings.contains("q"));
    }
}
