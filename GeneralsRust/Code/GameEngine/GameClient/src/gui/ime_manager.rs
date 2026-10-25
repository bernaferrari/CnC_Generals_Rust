//! IME manager (cross-platform state manager).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::game_window::WindowEventEffects;
use super::game_window::{GameWindow, WindowMessage, WindowStatus};
use super::window_manager::WindowManager;

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

/// Native window association capability installed on the receiving client.
pub type ImeOsAssociateHook = fn(associate: bool);

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
    last_action: ResidualImeAction,
    os_associate: Option<ImeOsAssociateHook>,
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
            last_action: ResidualImeAction::None,
            os_associate: None,
        }
    }

    pub fn set_os_associate_hook(&mut self, hook: ImeOsAssociateHook) {
        self.os_associate = Some(hook);
    }
    fn associate_os_ime(&self, associate: bool) {
        if let Some(hook) = self.os_associate {
            hook(associate);
        }
    }
    pub fn reset(&mut self, manager: &mut WindowManager) {
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
        self.close_candidate_list(manager);
    }

    pub fn update(&mut self) {}

    /// C++ `IMEManager::attach`: associate the IMM context when enabled.
    pub fn attach(&mut self, window: Rc<RefCell<GameWindow>>) {
        if self.is_attached_to(&window) {
            return;
        }
        self.detach();
        if self.disabled <= 0 {
            self.associate_os_ime(true);
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
            self.associate_os_ime(true);
        }
    }

    /// C++ `IMEManager::disable`: increment disable count and drop the IMM context.
    pub fn disable(&mut self) {
        self.disabled += 1;
        self.enabled = false;
        self.composing = false;
        self.associate_os_ime(false);
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

    /// C++ `IMEManager::closeCandidateList`.
    pub fn close_candidate_list(&mut self, manager: &mut WindowManager) {
        if let Some(window) = self
            .candidate_window
            .as_ref()
            .and_then(|weak| weak.upgrade())
        {
            let _ = manager.hide_window(&window, true);
            let _ = manager.unset_modal(&window);
        }
        self.candidates.clear();
        self.candidate_count_reset();
    }

    fn candidate_count_reset(&mut self) {
        self.page_start = 0;
        self.page_size = 0;
        self.selected_index = 0;
    }

    pub fn prepare_ime_message(
        &mut self,
        message: ImeMessage,
        manager: &mut WindowManager,
    ) -> ImeMessageDelivery {
        let mut delivery = ImeMessageDelivery {
            target: self.window.clone(),
            text: String::new(),
            handled: self.enabled,
            open_candidate_window: false,
        };
        if !self.enabled {
            return delivery;
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
                delivery.text = text;
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
                delivery.open_candidate_window = true;
            }
            ImeMessage::ClearCandidateList => {
                self.close_candidate_list(manager);
            }
        }

        delivery
    }
}

/// Owned continuation: no IME borrow remains while callbacks deliver committed text.
pub struct ImeMessageDelivery {
    target: Option<Weak<RefCell<GameWindow>>>,
    text: String,
    handled: bool,
    open_candidate_window: bool,
}
impl ImeMessageDelivery {
    pub fn deliver(
        self,
        manager: &mut WindowManager,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        if self.open_candidate_window {
            open_candidate_list_receiving(manager, effects);
        }
        if let Some(window) = self.target.and_then(|target| target.upgrade()) {
            for ch in self
                .text
                .chars()
                .filter(|ch| (*ch as u32) > 32 || *ch == '\r' || *ch == '\n')
            {
                effects.selected_window = Some(Rc::downgrade(&window));
                effects.with_receiving_manager(manager, |effects| {
                    window.borrow_mut().send_input_message_with_message_sink(
                        WindowMessage::ImeChar,
                        ch as u32 as crate::gui::WindowMsgData,
                        0,
                        false,
                        effects,
                        None,
                    );
                });
            }
        }
        self.handled
    }
}

/// Initialize the receiving IME without holding its state borrow across
/// authored candidate-window Create/ScriptCreate callbacks.
pub fn initialize_receiving_ime(manager: &mut WindowManager, effects: &mut WindowEventEffects<'_>) {
    effects.ime_mut().reset(manager);
    ensure_candidate_window_receiving(manager, effects);
}
fn ensure_candidate_window_receiving(
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Option<Rc<RefCell<GameWindow>>> {
    if let Some(window) = effects
        .ime()
        .candidate_window
        .as_ref()
        .and_then(Weak::upgrade)
    {
        return Some(window);
    }
    let info = manager
        .create_windows_from_script("IMECandidateWindow.wnd", effects)
        .ok()?;
    let window = info.windows.first()?.clone();
    let _ = window.borrow_mut().set_status(WindowStatus::ABOVE);
    let _ = manager.hide_window(&window, true);
    effects.ime_mut().candidate_window = Some(Rc::downgrade(&window));
    Some(window)
}
/// Finish at the original synchronous point after releasing the IME state loan.
fn open_candidate_list_receiving(
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) {
    let Some(window) = ensure_candidate_window_receiving(manager, effects) else {
        return;
    };
    let _ = manager.hide_window(&window, false);
    let _ = window.borrow_mut().set_status(WindowStatus::ABOVE);
    let _ = manager.set_modal(window.clone());
    let (width, _) = manager.screen_size();
    let (candidate_width, _) = window.borrow().get_size();
    let _ = window
        .borrow_mut()
        .set_position((width - candidate_width).max(0), 0);
}

pub fn attach_or_detach_for_focus(
    ime: &mut ImeManager,
    window: Rc<RefCell<GameWindow>>,
    focused: bool,
) {
    if focused {
        ime.attach(window);
    } else if ime.is_attached_to(&window) {
        ime.attach_optional(None);
    }
}
pub fn ime_should_swallow_input_for_window(ime: &ImeManager, window_id: i32) -> bool {
    ime.is_composing() && ime.is_attached_to_id(window_id)
}
pub fn service_ime_message(message: ImeMessage, effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_manager(|manager, effects| {
        let Some(ime) = effects.ime.as_deref_mut() else {
            return false;
        };
        let delivery = ime.prepare_ime_message(message, manager);
        delivery.deliver(manager, effects)
    })
}
/// OS ingress requires the same receiving GUI services as normal input dispatch.
pub fn service_os_ime_message(
    message: u32,
    wparam: usize,
    _lparam: isize,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    match message {
        WM_IME_STARTCOMPOSITION => service_ime_message(ImeMessage::StartComposition, effects),
        WM_IME_ENDCOMPOSITION => service_ime_message(ImeMessage::EndComposition, effects),
        WM_IME_CHAR | WM_CHAR => {
            let Some(ch) = char::from_u32((wparam as u32) & 0xffff) else {
                return false;
            };
            effects.with_manager(|manager, effects| {
                let Some(ime) = effects.ime.as_deref() else {
                    return false;
                };
                let delivery = ImeMessageDelivery {
                    target: ime.window.clone(),
                    text: ch.to_string(),
                    handled: true,
                    open_candidate_window: false,
                };
                delivery.deliver(manager, effects)
            })
        }
        WM_IME_NOTIFY => effects.with_manager(|manager, effects| {
            if effects.ime.is_none() {
                return false;
            }
            match wparam {
                IMN_OPENCANDIDATE | IMN_CHANGECANDIDATE => {
                    open_candidate_list_receiving(manager, effects)
                }
                IMN_CLOSECANDIDATE => effects.ime_mut().close_candidate_list(manager),
                _ => return false,
            }
            true
        }),
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

pub fn residual_ime_last_action(ime: &ImeManager) -> ResidualImeAction {
    ime.last_action
}
pub fn residual_ime_is_enabled(ime: &ImeManager) -> bool {
    ime.is_enabled()
}
pub fn residual_ime_is_composing(ime: &ImeManager) -> bool {
    ime.is_composing()
}
pub fn residual_ime_candidate_count(ime: &ImeManager) -> usize {
    ime.candidate_count()
}

fn perform_ime(
    message: ImeMessage,
    action: ResidualImeAction,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    let handled = service_ime_message(message, effects);
    if handled {
        if let Some(ime) = effects.ime.as_deref_mut() {
            ime.last_action = action;
        }
    }
    handled
}
pub fn simulate_ime_enable(effects: &mut WindowEventEffects<'_>) -> bool {
    let Some(ime) = effects.ime.as_deref_mut() else {
        return false;
    };
    ime.enable();
    ime.last_action = ResidualImeAction::Enable;
    ime.is_enabled()
}
pub fn simulate_ime_disable(effects: &mut WindowEventEffects<'_>) -> bool {
    let Some(ime) = effects.ime.as_deref_mut() else {
        return false;
    };
    ime.disable();
    ime.last_action = ResidualImeAction::Disable;
    !ime.is_enabled()
}
pub fn simulate_ime_start_composition(effects: &mut WindowEventEffects<'_>) -> bool {
    perform_ime(
        ImeMessage::StartComposition,
        ResidualImeAction::StartComposition,
        effects,
    )
}
pub fn simulate_ime_update_composition(
    text: &str,
    cursor_pos: usize,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    perform_ime(
        ImeMessage::UpdateComposition {
            text: text.to_owned(),
            cursor_pos,
        },
        ResidualImeAction::UpdateComposition,
        effects,
    )
}
pub fn simulate_ime_result_string(text: &str, effects: &mut WindowEventEffects<'_>) -> bool {
    perform_ime(
        ImeMessage::ResultString(text.to_owned()),
        ResidualImeAction::ResultString,
        effects,
    )
}
pub fn simulate_ime_candidate_list(
    candidates: &[&str],
    selected_index: usize,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    perform_ime(
        ImeMessage::CandidateList {
            candidates: candidates.iter().map(|s| (*s).to_owned()).collect(),
            selected_index,
            page_start: 0,
            page_size: candidates.len().max(1),
            index_base: 1,
        },
        ResidualImeAction::CandidateList,
        effects,
    )
}
pub fn simulate_ime_clear_candidates(effects: &mut WindowEventEffects<'_>) -> bool {
    perform_ime(
        ImeMessage::ClearCandidateList,
        ResidualImeAction::ClearCandidates,
        effects,
    )
}
pub fn simulate_ime_end_composition(effects: &mut WindowEventEffects<'_>) -> bool {
    perform_ime(
        ImeMessage::EndComposition,
        ResidualImeAction::EndComposition,
        effects,
    )
}
pub fn simulate_ime_reset(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_manager(|manager, effects| {
        let Some(ime) = effects.ime.as_deref_mut() else {
            return false;
        };
        ime.reset(manager);
        ime.last_action = ResidualImeAction::Reset;
        true
    })
}
pub fn simulate_ime_prepare_composition_cycle(
    text: &str,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_ime_enable(effects)
        && simulate_ime_start_composition(effects)
        && simulate_ime_update_composition(text, text.chars().count(), effects)
        && simulate_ime_candidate_list(&["a", "b", "c"], 0, effects)
}

/// C++ `IMEManager::openCandidateList` gadgets from `IMECandidateWindow.wnd`.
const IME_CANDIDATE_GADGETS: &[&str] = &[
    "IMECandidateWindow.wnd:TextArea",
    "IMECandidateWindow.wnd:UpArrow",
    "IMECandidateWindow.wnd:DownArrow",
];

fn click_any_ime_candidate_gadget(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    IME_CANDIDATE_GADGETS
        .iter()
        .any(|name| crate::gui::dispatch_os_click_named_window(name, effects))
}

/// Human click-through: OS LeftDown/Up on `IMECandidateWindow.wnd:TextArea`
/// (C++ candidate list hit) then composition cycle residual.
pub fn drive_os_wnd_ime_prepare_composition_cycle_like_cpp(
    text: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if text.is_empty() {
        return false;
    }
    if !click_any_ime_candidate_gadget(effects) {
        return false;
    }
    simulate_ime_prepare_composition_cycle(text, effects)
}

pub fn drive_os_wnd_ime_clear_candidates_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !click_any_ime_candidate_gadget(effects) {
        return false;
    }
    simulate_ime_clear_candidates(effects)
}

pub fn drive_os_wnd_ime_result_like_cpp(
    text: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !click_any_ime_candidate_gadget(effects) {
        return false;
    }
    simulate_ime_result_string(text, effects)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    fn install_named_button(
        client: &mut crate::core::game_client::GameClient,
        name: &str,
        x: i32,
        y: i32,
    ) {
        client.window_manager_owner().with_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }
    #[test]
    fn os_wnd_ime_prepare_hits_textarea_then_opens_candidate_list() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        install_named_button(&mut client, "IMECandidateWindow.wnd:TextArea", 10, 10);
        assert!(
            client.with_window_event_effects(None, |effects| {
                drive_os_wnd_ime_prepare_composition_cycle_like_cpp("nihao", effects)
            }),
            "OS WND click on IME TextArea must run composition cycle residual"
        );
        assert_eq!(
            residual_ime_last_action(client.ime()),
            ResidualImeAction::CandidateList
        );
        assert_eq!(residual_ime_candidate_count(client.ime()), 3);
        assert!(residual_ime_is_composing(client.ime()));
        assert!(!client.with_window_event_effects(None, |effects| {
            drive_os_wnd_ime_prepare_composition_cycle_like_cpp("", effects)
        }));
    }
    #[test]
    fn os_wnd_ime_clear_hits_downarrow_then_clears_candidates() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        install_named_button(&mut client, "IMECandidateWindow.wnd:DownArrow", 10, 40);
        client.with_window_event_effects(None, |effects| {
            let _ = simulate_ime_candidate_list(&["a", "b"], 0, effects);
            assert!(drive_os_wnd_ime_clear_candidates_like_cpp(effects));
        });
        assert_eq!(
            residual_ime_last_action(client.ime()),
            ResidualImeAction::ClearCandidates
        );
        assert_eq!(residual_ime_candidate_count(client.ime()), 0);
    }
}
