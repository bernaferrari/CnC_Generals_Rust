//! IME manager (cross-platform state manager).

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

use super::game_window::GameWindow;

#[derive(Debug, Clone)]
pub enum ImeMessage {
    StartComposition,
    EndComposition,
    UpdateComposition {
        text: String,
        cursor_pos: usize,
    },
    ResultString(String),
    CandidateList {
        candidates: Vec<String>,
        selected_index: usize,
        page_start: usize,
        page_size: usize,
        index_base: i32,
    },
    ClearCandidateList,
}

#[derive(Debug)]
pub struct ImeManager {
    window: Option<Weak<RefCell<GameWindow>>>,
    enabled: bool,
    composing: bool,
    composition_string: String,
    result_string: String,
    composition_cursor_pos: usize,
    index_base: i32,
    page_start: usize,
    page_size: usize,
    selected_index: usize,
    candidates: Vec<String>,
}

impl Default for ImeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ImeManager {
    pub fn new() -> Self {
        Self {
            window: None,
            enabled: true,
            composing: false,
            composition_string: String::new(),
            result_string: String::new(),
            composition_cursor_pos: 0,
            index_base: 0,
            page_start: 0,
            page_size: 0,
            selected_index: 0,
            candidates: Vec::new(),
        }
    }

    pub fn init(&mut self) {
        self.reset();
    }

    pub fn reset(&mut self) {
        self.window = None;
        self.composing = false;
        self.composition_string.clear();
        self.result_string.clear();
        self.candidates.clear();
        self.composition_cursor_pos = 0;
        self.index_base = 0;
        self.page_start = 0;
        self.page_size = 0;
        self.selected_index = 0;
    }

    pub fn update(&mut self) {}

    pub fn attach(&mut self, window: Rc<RefCell<GameWindow>>) {
        self.window = Some(Rc::downgrade(&window));
    }

    pub fn detach(&mut self) {
        self.window = None;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
        self.composing = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_attached_to(&self, window: &Rc<RefCell<GameWindow>>) -> bool {
        self.window
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|w| Rc::ptr_eq(&w, window))
            .unwrap_or(false)
    }

    pub fn get_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.window.as_ref().and_then(|weak| weak.upgrade())
    }

    pub fn is_composing(&self) -> bool {
        self.composing
    }

    pub fn composition_string(&self) -> &str {
        &self.composition_string
    }

    pub fn composition_cursor_position(&self) -> usize {
        self.composition_cursor_pos
    }

    pub fn index_base(&self) -> i32 {
        self.index_base
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    pub fn candidate(&self, index: usize) -> Option<&str> {
        self.candidates.get(index).map(|s| s.as_str())
    }

    pub fn selected_candidate_index(&self) -> usize {
        self.selected_index
    }

    pub fn candidate_page_size(&self) -> usize {
        self.page_size
    }

    pub fn candidate_page_start(&self) -> usize {
        self.page_start
    }

    pub fn result_string(&self) -> &str {
        &self.result_string
    }

    pub fn service_ime_message(&mut self, message: ImeMessage) -> bool {
        if !self.enabled {
            return false;
        }

        match message {
            ImeMessage::StartComposition => {
                self.composing = true;
                self.composition_string.clear();
                self.result_string.clear();
            }
            ImeMessage::EndComposition => {
                self.composing = false;
            }
            ImeMessage::UpdateComposition { text, cursor_pos } => {
                self.composition_string = text;
                self.composition_cursor_pos = cursor_pos;
                self.composing = true;
            }
            ImeMessage::ResultString(text) => {
                self.result_string = text;
                self.composing = false;
            }
            ImeMessage::CandidateList {
                candidates,
                selected_index,
                page_start,
                page_size,
                index_base,
            } => {
                self.candidates = candidates;
                self.selected_index = selected_index;
                self.page_start = page_start;
                self.page_size = page_size;
                self.index_base = index_base;
            }
            ImeMessage::ClearCandidateList => {
                self.candidates.clear();
                self.page_start = 0;
                self.page_size = 0;
                self.selected_index = 0;
            }
        }

        true
    }
}

thread_local! {
    static THE_IME_MANAGER: Arc<Mutex<ImeManager>> = Arc::new(Mutex::new(ImeManager::new()));
}

pub fn get_ime_manager() -> Arc<Mutex<ImeManager>> {
    THE_IME_MANAGER.with(|manager| manager.clone())
}

/// Residual: last IME action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualImeAction {
    None = 0,
    Enable = 1,
    Disable = 2,
    StartComposition = 3,
    UpdateComposition = 4,
    ResultString = 5,
    CandidateList = 6,
    ClearCandidates = 7,
    EndComposition = 8,
    Reset = 9,
}

static RESIDUAL_IME_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_IME_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static RESIDUAL_IME_COMPOSING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_IME_CANDIDATE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn residual_ime_action_store(action: ResidualImeAction) {
    RESIDUAL_IME_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

fn residual_ime_sync_flags(manager: &ImeManager) {
    RESIDUAL_IME_ENABLED.store(manager.is_enabled(), std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_IME_COMPOSING.store(manager.is_composing(), std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_IME_CANDIDATE_COUNT.store(
        manager.candidate_count(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// Residual: last IME residual action.
pub fn residual_ime_last_action() -> ResidualImeAction {
    match RESIDUAL_IME_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualImeAction::Enable,
        2 => ResidualImeAction::Disable,
        3 => ResidualImeAction::StartComposition,
        4 => ResidualImeAction::UpdateComposition,
        5 => ResidualImeAction::ResultString,
        6 => ResidualImeAction::CandidateList,
        7 => ResidualImeAction::ClearCandidates,
        8 => ResidualImeAction::EndComposition,
        9 => ResidualImeAction::Reset,
        _ => ResidualImeAction::None,
    }
}

/// Residual: IME enabled latch.
pub fn residual_ime_is_enabled() -> bool {
    RESIDUAL_IME_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: IME composing latch.
pub fn residual_ime_is_composing() -> bool {
    RESIDUAL_IME_COMPOSING.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: candidate count latch.
pub fn residual_ime_candidate_count() -> usize {
    RESIDUAL_IME_CANDIDATE_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: enable IME without window attach.
pub fn simulate_ime_enable() -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        guard.enable();
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::Enable);
        return residual_ime_is_enabled();
    }
    false
}

/// Residual: disable IME without window attach.
pub fn simulate_ime_disable() -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        guard.disable();
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::Disable);
        return !residual_ime_is_enabled();
    }
    false
}

/// Residual: start composition without OS IME.
pub fn simulate_ime_start_composition() -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        if !guard.service_ime_message(ImeMessage::StartComposition) {
            return false;
        }
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::StartComposition);
        return residual_ime_is_composing();
    }
    false
}

/// Residual: update composition text without OS IME.
pub fn simulate_ime_update_composition(text: &str, cursor_pos: usize) -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        if !guard.service_ime_message(ImeMessage::UpdateComposition {
            text: text.to_string(),
            cursor_pos,
        }) {
            return false;
        }
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::UpdateComposition);
        return residual_ime_is_composing() && guard.composition_string() == text;
    }
    false
}

/// Residual: commit result string without OS IME.
pub fn simulate_ime_result_string(text: &str) -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        if !guard.service_ime_message(ImeMessage::ResultString(text.to_string())) {
            return false;
        }
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::ResultString);
        return guard.result_string() == text && !residual_ime_is_composing();
    }
    false
}

/// Residual: push candidate list without OS IME.
pub fn simulate_ime_candidate_list(candidates: &[&str], selected_index: usize) -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        if !guard.service_ime_message(ImeMessage::CandidateList {
            candidates: candidates.iter().map(|s| (*s).to_string()).collect(),
            selected_index,
            page_start: 0,
            page_size: candidates.len().max(1),
            index_base: 1,
        }) {
            return false;
        }
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::CandidateList);
        return residual_ime_candidate_count() == candidates.len();
    }
    false
}

/// Residual: clear candidate list without OS IME.
pub fn simulate_ime_clear_candidates() -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        if !guard.service_ime_message(ImeMessage::ClearCandidateList) {
            return false;
        }
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::ClearCandidates);
        return residual_ime_candidate_count() == 0;
    }
    false
}

/// Residual: end composition without OS IME.
pub fn simulate_ime_end_composition() -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        if !guard.service_ime_message(ImeMessage::EndComposition) {
            return false;
        }
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::EndComposition);
        return !residual_ime_is_composing();
    }
    false
}

/// Residual: reset IME manager residual.
pub fn simulate_ime_reset() -> bool {
    let manager = get_ime_manager();
    if let Ok(mut guard) = manager.lock() {
        guard.reset();
        residual_ime_sync_flags(&guard);
        residual_ime_action_store(ResidualImeAction::Reset);
        return true;
    }
    false
}

/// Residual: enable + composition + candidates composite.
pub fn simulate_ime_prepare_composition_cycle(text: &str) -> bool {
    if !simulate_ime_enable() {
        return false;
    }
    if !simulate_ime_start_composition() {
        return false;
    }
    if !simulate_ime_update_composition(text, text.chars().count()) {
        return false;
    }
    if !simulate_ime_candidate_list(&["a", "b", "c"], 0) {
        return false;
    }
    true
}

/// C++ `IMEManager::openCandidateList` gadgets from `IMECandidateWindow.wnd`.
const IME_CANDIDATE_GADGETS: &[&str] = &[
    "IMECandidateWindow.wnd:TextArea",
    "IMECandidateWindow.wnd:UpArrow",
    "IMECandidateWindow.wnd:DownArrow",
];

fn click_any_ime_candidate_gadget() -> bool {
    IME_CANDIDATE_GADGETS
        .iter()
        .any(|name| crate::gui::dispatch_os_click_named_window(name))
}

/// Human click-through: OS LeftDown/Up on `IMECandidateWindow.wnd:TextArea`
/// (C++ candidate list hit) then composition cycle residual.
pub fn drive_os_wnd_ime_prepare_composition_cycle_like_cpp(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if !click_any_ime_candidate_gadget() {
        return false;
    }
    simulate_ime_prepare_composition_cycle(text)
}

pub fn drive_os_wnd_ime_clear_candidates_like_cpp() -> bool {
    if !click_any_ime_candidate_gadget() {
        return false;
    }
    simulate_ime_clear_candidates()
}

pub fn drive_os_wnd_ime_result_like_cpp(text: &str) -> bool {
    if !click_any_ime_candidate_gadget() {
        return false;
    }
    simulate_ime_result_string(text)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    #[test]
    fn os_wnd_ime_prepare_hits_textarea_then_opens_candidate_list() {
        install_named_button("IMECandidateWindow.wnd:TextArea", 10, 10);
        assert!(
            drive_os_wnd_ime_prepare_composition_cycle_like_cpp("nihao"),
            "OS WND click on IME TextArea must run composition cycle residual"
        );
        assert_eq!(residual_ime_last_action(), ResidualImeAction::CandidateList);
        assert_eq!(residual_ime_candidate_count(), 3);
        assert!(residual_ime_is_composing());
        assert!(!drive_os_wnd_ime_prepare_composition_cycle_like_cpp(""));
    }

    #[test]
    fn os_wnd_ime_clear_hits_downarrow_then_clears_candidates() {
        install_named_button("IMECandidateWindow.wnd:DownArrow", 10, 40);
        let _ = simulate_ime_candidate_list(&["a", "b"], 0);
        assert!(drive_os_wnd_ime_clear_candidates_like_cpp());
        assert_eq!(
            residual_ime_last_action(),
            ResidualImeAction::ClearCandidates
        );
        assert_eq!(residual_ime_candidate_count(), 0);
    }
}
