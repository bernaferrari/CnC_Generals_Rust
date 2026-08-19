//! IME manager (cross-platform state manager).

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex, OnceLock};

use super::game_window::{GameWindow, WindowMessage, WindowStatus};
use super::window_manager::with_window_manager;

/// C++ `WM_IME_*` / `WM_CHAR` codes serviced by `IMEManager::serviceIMEMessage`.
pub const WM_CHAR: u32 = 0x0102;
pub const WM_IME_STARTCOMPOSITION: u32 = 0x010D;
pub const WM_IME_ENDCOMPOSITION: u32 = 0x010E;
pub const WM_IME_COMPOSITION: u32 = 0x010F;
pub const WM_IME_SETCONTEXT: u32 = 0x0281;
pub const WM_IME_NOTIFY: u32 = 0x0282;
pub const WM_IME_CHAR: u32 = 0x0286;
pub const IMN_CHANGECANDIDATE: usize = 0x0003;
pub const IMN_CLOSECANDIDATE: usize = 0x0004;
pub const IMN_OPENCANDIDATE: usize = 0x0005;

type ImeOsAssociateHook = fn(associate: bool);
static IME_OS_ASSOCIATE: OnceLock<ImeOsAssociateHook> = OnceLock::new();

/// Install the platform ImmAssociateContext / IME-enable hook.
pub fn set_ime_os_associate_hook(hook: ImeOsAssociateHook) {
    let _ = IME_OS_ASSOCIATE.set(hook);
}

fn associate_os_ime(associate: bool) {
    if let Some(hook) = IME_OS_ASSOCIATE.get() {
        hook(associate);
    }
}

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
    disabled: i32,
    composing: bool,
    composition_string: String,
    result_string: String,
    composition_cursor_pos: usize,
    index_base: i32,
    page_start: usize,
    page_size: usize,
    selected_index: usize,
    candidates: Vec<String>,
    candidate_window: Option<Weak<RefCell<GameWindow>>>,
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
            disabled: 0,
            composing: false,
            composition_string: String::new(),
            result_string: String::new(),
            composition_cursor_pos: 0,
            index_base: 0,
            page_start: 0,
            page_size: 0,
            selected_index: 0,
            candidates: Vec::new(),
            candidate_window: None,
        }
    }

    pub fn init(&mut self) {
        self.reset();
        self.ensure_candidate_window();
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
        self.close_candidate_list();
    }

    pub fn update(&mut self) {}

    /// C++ `IMEManager::attach`: associate the IMM context when enabled.
    pub fn attach(&mut self, window: Rc<RefCell<GameWindow>>) {
        if self.is_attached_to(&window) {
            return;
        }
        self.detach();
        if self.disabled <= 0 {
            associate_os_ime(true);
        }
        self.window = Some(Rc::downgrade(&window));
    }

    pub fn attach_optional(&mut self, window: Option<Rc<RefCell<GameWindow>>>) {
        match window {
            Some(window) => self.attach(window),
            None => self.detach(),
        }
    }

    pub fn detach(&mut self) {
        self.window = None;
    }

    /// C++ `IMEManager::enable`: decrement disable count and ImmAssociateContext.
    pub fn enable(&mut self) {
        self.disabled -= 1;
        if self.disabled <= 0 {
            self.disabled = 0;
            self.enabled = true;
            associate_os_ime(true);
        }
    }

    /// C++ `IMEManager::disable`: increment disable count and drop the IMM context.
    pub fn disable(&mut self) {
        self.disabled += 1;
        self.enabled = false;
        self.composing = false;
        associate_os_ime(false);
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

    pub fn is_attached_to_id(&self, window_id: i32) -> bool {
        self.window
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|w| w.borrow().get_id() == window_id)
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

    fn dispatch_ime_chars(&self, text: &str) {
        let Some(window) = self.get_window() else {
            return;
        };
        for ch in text.chars() {
            if (ch as u32) > 32 || ch == '\r' || ch == '\n' {
                let _ = window.borrow_mut().send_input_message(
                    WindowMessage::ImeChar,
                    ch as u32 as crate::gui::game_window::WindowMsgData,
                    0,
                );
            }
        }
    }

    pub fn dispatch_ime_char(&self, ch: char) {
        if (ch as u32) > 32 || ch == '\r' || ch == '\n' {
            if let Some(window) = self.get_window() {
                let _ = window.borrow_mut().send_input_message(
                    WindowMessage::ImeChar,
                    ch as u32 as crate::gui::game_window::WindowMsgData,
                    0,
                );
            }
        }
    }

    fn ensure_candidate_window(&mut self) {
        if self
            .candidate_window
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .is_some()
        {
            return;
        }
        with_window_manager(|manager| {
            if let Ok(info) = manager.create_windows_from_script("IMECandidateWindow.wnd") {
                if let Some(window) = info.windows.first() {
                    let _ = window.borrow_mut().set_status(WindowStatus::ABOVE);
                    let _ = window.borrow_mut().hide(true);
                    self.candidate_window = Some(Rc::downgrade(window));
                }
            }
        });
    }

    /// C++ `IMEManager::openCandidateList`.
    pub fn open_candidate_list(&mut self) {
        self.ensure_candidate_window();
        let Some(window) = self
            .candidate_window
            .as_ref()
            .and_then(|weak| weak.upgrade())
        else {
            return;
        };
        let _ = window.borrow_mut().hide(false);
        let _ = window.borrow_mut().set_status(WindowStatus::ABOVE);
        with_window_manager(|manager| {
            let _ = manager.set_modal(window.clone());
        });
        let (cx, cy) = with_window_manager(|manager| {
            let (width, _) = manager.screen_size();
            let (cwidth, _) = window.borrow().get_size();
            ((width - cwidth).max(0), 0)
        });
        let _ = window.borrow_mut().set_position(cx, cy);
    }

    /// C++ `IMEManager::closeCandidateList`.
    pub fn close_candidate_list(&mut self) {
        if let Some(window) = self
            .candidate_window
            .as_ref()
            .and_then(|weak| weak.upgrade())
        {
            let _ = window.borrow_mut().hide(true);
            with_window_manager(|manager| {
                let _ = manager.unset_modal(&window);
            });
        }
        self.candidates.clear();
        self.candidate_count_reset();
    }

    fn candidate_count_reset(&mut self) {
        self.page_start = 0;
        self.page_size = 0;
        self.selected_index = 0;
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
                if let Some(window) = self.get_window() {
                    if let Some(entry) = window.borrow_mut().text_entry_mut() {
                        entry.set_ime_composition(
                            self.composition_string.clone(),
                            self.composition_cursor_pos,
                        );
                    }
                }
            }
            ImeMessage::ResultString(text) => {
                self.result_string = text.clone();
                self.composing = false;
                if let Some(window) = self.get_window() {
                    if let Some(entry) = window.borrow_mut().text_entry_mut() {
                        entry.set_ime_composition(String::new(), 0);
                    }
                }
                self.dispatch_ime_chars(&text);
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
                self.open_candidate_list();
            }
            ImeMessage::ClearCandidateList => {
                self.close_candidate_list();
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

/// C++ GadgetTextEntrySystem GWM_INPUT_FOCUS attach / detach.
pub fn attach_or_detach_for_focus(window: Rc<RefCell<GameWindow>>, focused: bool) {
    if let Ok(mut manager) = get_ime_manager().lock() {
        if focused {
            manager.attach(window);
        } else if manager.is_attached_to(&window) {
            manager.attach_optional(None);
        }
    }
}

/// C++ GadgetTextEntryInput swallows keys while IME is composing on this window.
pub fn ime_should_swallow_input_for_window(window_id: i32) -> bool {
    get_ime_manager()
        .lock()
        .map(|manager| manager.is_composing() && manager.is_attached_to_id(window_id))
        .unwrap_or(false)
}

/// C++ `IMEManager::serviceIMEMessage` for Win32 `WM_IME_*` / `WM_CHAR`.
pub fn service_os_ime_message(message: u32, wparam: usize, _lparam: isize) -> bool {
    let manager = get_ime_manager();
    let Ok(mut guard) = manager.lock() else {
        return false;
    };
    match message {
        WM_IME_STARTCOMPOSITION => guard.service_ime_message(ImeMessage::StartComposition),
        WM_IME_ENDCOMPOSITION => guard.service_ime_message(ImeMessage::EndComposition),
        WM_IME_CHAR | WM_CHAR => {
            if let Some(ch) = char::from_u32((wparam as u32) & 0xffff) {
                guard.dispatch_ime_char(ch);
                true
            } else {
                false
            }
        }
        WM_IME_NOTIFY => match wparam {
            IMN_OPENCANDIDATE | IMN_CHANGECANDIDATE => {
                guard.open_candidate_list();
                true
            }
            IMN_CLOSECANDIDATE => {
                guard.close_candidate_list();
                true
            }
            _ => false,
        },
        _ => false,
    }
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
