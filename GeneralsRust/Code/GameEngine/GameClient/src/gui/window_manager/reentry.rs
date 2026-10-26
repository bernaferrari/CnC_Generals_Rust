//! Receiving owner callback queues and explicit OS input dispatch.
#![allow(unused_imports)]

use crate::gui::game_window::*;
use crate::gui::load_screen::{LoadScreenPreludeOutcome, LoadScreenPreludeStep};
use crate::gui::shell::{ResidualShellMapAction, Shell};
use crate::gui::w3d_gadget_draw::{
    w3d_main_menu_button_drop_shadow_draw, w3d_main_menu_random_text_draw,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use super::*;

enum WindowManagerOperation {
    WithEffects(
        Box<
            dyn for<'a> FnOnce(
                    &mut WindowManager,
                    &mut crate::gui::game_window::WindowEventEffects<'a>,
                ) + 'static,
        >,
    ),
}
enum ShellOperation {
    WithEffects(
        Box<
            dyn for<'a> FnOnce(&mut Shell, &mut crate::gui::game_window::WindowEventEffects<'a>)
                + 'static,
        >,
    ),
}

struct WindowManagerOwnerState {
    manager: RefCell<WindowManager>,
    operations: RefCell<Vec<WindowManagerOperation>>,
    replay_steps: RefCell<VecDeque<crate::gui::callbacks::replay_menu::ingress::ReplayMenuStep>>,
    shell: RefCell<Shell>,
    shell_operations: RefCell<VecDeque<ShellOperation>>,
    pending_shell_scheme: RefCell<Option<String>>,
    shell_anim_finished: Cell<bool>,
    residual_shell_map_action: Cell<ResidualShellMapAction>,
    residual_shell_map_on: Cell<bool>,
    deferred_shell_pushes: RefCell<Vec<String>>,
    last_widget_tree_click_used: Cell<bool>,
    widget_tree_navigation_observed: Cell<bool>,
}

/// Persistent owner for one native client's WND tree and its re-entry queues.
/// The handle is single-threaded because WindowManager owns Rc-backed windows.
#[derive(Clone)]
pub struct WindowManagerOwner {
    state: Rc<WindowManagerOwnerState>,
}

/// Weak identity of one original UI tree, retained by an owned replay step.
#[derive(Clone, Debug)]
pub struct WindowManagerReceipt {
    state: Weak<WindowManagerOwnerState>,
}
impl WindowManagerReceipt {
    pub(crate) fn hide_registered_window(&self, window: Weak<RefCell<GameWindow>>) {
        let (Some(state), Some(window)) = (self.state.upgrade(), window.upgrade()) else {
            return;
        };
        // The admitted replay step runs only after callback and WM loans end.
        let mut manager = state.manager.borrow_mut();
        let id = window.borrow().get_id();
        let Some(current) = manager.get_window_by_id(id) else {
            return;
        };
        if Rc::ptr_eq(&current, &window) {
            let _ = manager.hide_window(&window, true);
        }
    }
}

impl WindowManagerOwner {
    pub fn downgrade(&self) -> WindowManagerReceipt {
        WindowManagerReceipt {
            state: Rc::downgrade(&self.state),
        }
    }
    pub(crate) fn queue_replay_step(
        &self,
        step: crate::gui::callbacks::replay_menu::ingress::ReplayMenuStep,
    ) {
        self.state.replay_steps.borrow_mut().push_back(step);
    }
    pub(crate) fn clear_replay_steps(&self) {
        self.state.replay_steps.borrow_mut().clear();
    }

    pub fn new() -> Self {
        Self {
            state: Rc::new(WindowManagerOwnerState {
                manager: RefCell::new(WindowManager::new()),
                operations: RefCell::new(Vec::new()),
                replay_steps: RefCell::new(VecDeque::new()),
                shell: RefCell::new(Shell::new()),
                shell_operations: RefCell::new(VecDeque::new()),
                pending_shell_scheme: RefCell::new(None),
                shell_anim_finished: Cell::new(true),
                residual_shell_map_action: Cell::new(ResidualShellMapAction::None),
                residual_shell_map_on: Cell::new(false),
                deferred_shell_pushes: RefCell::new(Vec::new()),
                last_widget_tree_click_used: Cell::new(false),
                widget_tree_navigation_observed: Cell::new(false),
            }),
        }
    }

    pub fn last_widget_tree_click_used(&self) -> bool {
        self.state.last_widget_tree_click_used.get()
    }
    pub fn widget_tree_navigation_observed(&self) -> bool {
        self.state.widget_tree_navigation_observed.get()
    }
    pub(crate) fn record_widget_tree_click_used(&self, used: bool) {
        self.state.last_widget_tree_click_used.set(used);
    }
    pub(crate) fn record_widget_tree_navigation(&self) {
        self.state.widget_tree_navigation_observed.set(true);
    }
    #[cfg(test)]
    pub(crate) fn reset_widget_tree_observations(&self) {
        self.state.last_widget_tree_click_used.set(false);
        self.state.widget_tree_navigation_observed.set(false);
    }

    /// Called only after the driving host released all input/lifecycle loans.
    /// Old-session continuations must not mutate a replacement world or WND tree.
    pub(crate) fn discard_pending_event_operations_for_world_boundary(&self) {
        self.state.operations.borrow_mut().clear();
        self.state.shell_operations.borrow_mut().clear();
        self.state.deferred_shell_pushes.borrow_mut().clear();
    }

    /// Borrow this exact owner's manager for one synchronous operation.
    pub fn with_manager<R>(&self, f: impl FnOnce(&mut WindowManager) -> R) -> R {
        let mut manager = self.state.manager.borrow_mut();
        f(&mut manager)
    }

    /// Attempt a shared read of this owner's manager without selecting an
    /// ambient callback owner. `None` means the manager is currently mutably
    /// borrowed by a synchronous operation.
    pub fn try_with_manager_ref<R>(&self, f: impl FnOnce(&WindowManager) -> R) -> Option<R> {
        let owner = &self.state;
        let borrow = owner.manager.try_borrow().ok()?;
        Some(f(&borrow))
    }

    /// Attempt one owner-scoped mutable operation without a re-entry fallback.
    /// A live borrow means the callback must retry at its owning boundary.
    pub fn try_with_manager<R>(&self, f: impl FnOnce(&mut WindowManager) -> R) -> Option<R> {
        let mut manager = self.state.manager.try_borrow_mut().ok()?;
        Some(f(&mut manager))
    }

    pub(crate) fn queue_window_operation_with_effects(
        &self,
        operation: impl for<'a> FnOnce(
            &mut WindowManager,
            &mut crate::gui::game_window::WindowEventEffects<'a>,
        ) + 'static,
    ) {
        self.state
            .operations
            .borrow_mut()
            .push(WindowManagerOperation::WithEffects(Box::new(operation)));
    }

    pub(crate) fn drain_driving_shell(
        &self,
        shell: &mut Shell,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        drain_shell_operations_with_effects(&self.state, shell, effects);
    }

    pub(crate) fn complete_event_callbacks(
        &self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        loop {
            self.complete_window_callbacks(effects);
            self.with_shell_effects_mut(effects, |_, _| ())
                .expect("WND event must release receiving Shell before callback replay");
            // Core replay admission is a specific owned continuation. Release
            // every Window/WM/Shell loan before Recorder or UI service calls.
            let replay_step = self.state.replay_steps.borrow_mut().pop_front();
            if let Some(step) = replay_step {
                step.complete_compatibility(effects);
                continue;
            }
            if self.state.operations.borrow().is_empty()
                && self.state.shell_operations.borrow().is_empty()
            {
                return;
            }
        }
    }

    pub(crate) fn complete_window_callbacks(
        &self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let mut manager = self
            .state
            .manager
            .try_borrow_mut()
            .expect("WND callback replay requires outer manager loan released");
        drain_window_manager_ops_with_effects(&self.state, &mut manager, effects);
    }

    /// Read this exact owner's manager for one synchronous operation.
    pub fn with_manager_ref<R>(&self, f: impl FnOnce(&WindowManager) -> R) -> R {
        let manager = self.state.manager.borrow();
        f(&manager)
    }

    /// Mutate this client's Shell for one synchronous lifecycle operation.
    /// Nested callback work is drained before the sole mutable borrow ends.
    pub fn with_shell_mut<R>(&self, f: impl FnOnce(&mut Shell) -> R) -> Option<R> {
        let mut shell = self.state.shell.try_borrow_mut().ok()?;
        Some(f(&mut shell))
    }

    /// Complete the receiving client's queued lifecycle effects in their original event phase.
    pub(crate) fn with_shell_effects_mut<R>(
        &self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
        f: impl FnOnce(&mut Shell, &mut crate::gui::game_window::WindowEventEffects<'_>) -> R,
    ) -> Option<R> {
        let mut shell = self.state.shell.try_borrow_mut().ok()?;
        drain_shell_operations_with_effects(&self.state, &mut shell, effects);
        let result = f(&mut shell, effects);
        drain_shell_operations_with_effects(&self.state, &mut shell, effects);
        Some(result)
    }

    pub(crate) fn queue_shell_operation_with_effects(
        &self,
        operation: impl for<'a> FnOnce(&mut Shell, &mut crate::gui::game_window::WindowEventEffects<'a>)
        + 'static,
    ) {
        self.state
            .shell_operations
            .borrow_mut()
            .push_back(ShellOperation::WithEffects(Box::new(operation)));
    }

    /// Observe this owner's Shell without publishing a compatibility selector.
    pub fn shell_is_active(&self) -> Option<bool> {
        self.state
            .shell
            .try_borrow()
            .ok()
            .map(|shell| shell.is_shell_active())
    }

    /// Read this client's Shell when no mutable lifecycle operation is active.
    pub fn with_shell_ref<R>(&self, f: impl FnOnce(&Shell) -> R) -> Option<R> {
        let shell = self.state.shell.try_borrow().ok()?;
        Some(f(&shell))
    }

    /// Run or queue work on this owner's Shell, preserving the same re-entry boundary.

    /// Owner-local value read by layout callbacks during the Shell update borrow.
    pub(crate) fn shell_anim_finished(&self) -> bool {
        self.state.shell_anim_finished.get()
    }

    pub(crate) fn record_shell_anim_finished(&self, finished: bool) {
        self.state.shell_anim_finished.set(finished);
    }

    pub(crate) fn take_pending_shell_scheme(&self) -> Option<String> {
        self.state.pending_shell_scheme.borrow_mut().take()
    }

    pub(crate) fn set_residual_shell_map(
        &self,
        action: ResidualShellMapAction,
        shell_map_on: bool,
    ) {
        self.state.residual_shell_map_action.set(action);
        self.state.residual_shell_map_on.set(shell_map_on);
    }

    pub(crate) fn residual_shell_map_action(&self) -> ResidualShellMapAction {
        self.state.residual_shell_map_action.get()
    }

    pub(crate) fn residual_shell_map_on(&self) -> bool {
        self.state.residual_shell_map_on.get()
    }

    pub(crate) fn queue_deferred_shell_push(&self, screen: &str) -> bool {
        let mut queue = self.state.deferred_shell_pushes.borrow_mut();
        if queue.iter().any(|existing| existing == screen) {
            return false;
        }
        queue.push(screen.to_string());
        true
    }

    pub(crate) fn drain_deferred_shell_pushes(&self) -> Vec<String> {
        std::mem::take(&mut *self.state.deferred_shell_pushes.borrow_mut())
    }

    pub(crate) fn clear_deferred_shell_pushes(&self) -> usize {
        let mut queue = self.state.deferred_shell_pushes.borrow_mut();
        let count = queue.len();
        queue.clear();
        count
    }
}

impl Default for WindowManagerOwner {
    fn default() -> Self {
        Self::new()
    }
}

fn drain_shell_operations_with_effects(
    owner: &Rc<WindowManagerOwnerState>,
    shell: &mut Shell,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    loop {
        let operation = owner.shell_operations.borrow_mut().pop_front();
        match operation {
            Some(ShellOperation::WithEffects(operation)) => operation(shell, effects),
            None => return,
        }
    }
}

fn drain_window_manager_ops_with_effects(
    owner: &WindowManagerOwnerState,
    manager: &mut WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    loop {
        let op = {
            let mut operations = owner.operations.borrow_mut();
            if operations.is_empty() {
                return;
            }
            operations.remove(0)
        };
        match op {
            WindowManagerOperation::WithEffects(operation) => operation(manager, effects),
        }
    }
}

/// OS mouse → `TheWindowManager` hit-test + gadget input.
///
/// C++ `WindowXlat` turns `RAW_MOUSE_*` into `winSendInputMsg`. Shell active
/// consumes the click (LookAt/world command must not also fire).
///
/// Mouse-lock + scrolling LMB pass-through is the unused WindowXlat helper
/// (`WindowXlat.cpp:147-167` / `os_mouse_blocked_by_mouse_lock`): locked
/// view skips WM except LMB down/up while `TheInGameUI` is scrolling.
/// Immutable facts from the input owner for this operation. No active-view
/// selection occurs when Main dispatches its native WindowEvent stream.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowMouseFacts {
    pub mouse_locked: bool,
    pub scrolling: bool,
}

/// Dispatch a native mouse event to the explicitly receiving client's WND
/// tree. The owner is passed by Main at the OS-event boundary; the scoped
/// compatibility selection exists only while nested legacy gadget callbacks run.
pub fn dispatch_os_mouse_to_window_manager_for_owner(
    owner: &WindowManagerOwner,
    msg: WindowMessage,
    x: i32,
    y: i32,
    facts: WindowMouseFacts,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowInputReturnCode {
    dispatch_mouse_with_facts(
        msg,
        x,
        y,
        facts,
        |msg, x, y, data| {
            owner.with_manager(|manager| manager.process_mouse_event(msg, x, y, data, effects))
        },
        || {
            owner
                .with_shell_ref(Shell::is_shell_active)
                .unwrap_or(false)
        },
    )
}

pub(crate) fn opaque_window_at_screen(manager: &WindowManager, x: i32, y: i32) -> bool {
    let mut window = manager.get_window_under_cursor(x, y, false);
    while let Some(current) = window {
        let current = current.borrow();
        if !current.get_status().contains(WindowStatus::SEE_THRU) {
            return true;
        }
        window = current.get_parent();
    }
    false
}

/// Registered WindowTranslator input uses the same receiving UI owners as physical input.
pub fn dispatch_translated_window_input_for_owner(
    owner: &WindowManagerOwner,
    request: game_engine::common::message_stream::WindowInputDispatch,
    context: &mut super::ControlBarInputContext<'_>,
) -> Option<game_engine::common::message_stream::WindowInputDispatchResult> {
    if let game_engine::common::message_stream::WindowInputDispatch::OpaqueWindowAtScreen { x, y } =
        request
    {
        return owner.try_with_manager_ref(|manager| {
            game_engine::common::message_stream::WindowInputDispatchResult::OpaqueWindow(
                opaque_window_at_screen(manager, x, y),
            )
        });
    }
    if let game_engine::common::message_stream::WindowInputDispatch::Display(action) = request {
        let display = context.script_display.as_deref_mut()?;
        return crate::message_stream::meta_event::execute_display_input_action(
            action,
            display,
            owner,
            context.in_game_movies.as_deref_mut(),
        )
        .map(game_engine::common::message_stream::WindowInputDispatchResult::Display);
    }
    if let game_engine::common::message_stream::WindowInputDispatch::Camera(action) = request {
        return context
            .viewport
            .camera_input(action)
            .map(game_engine::common::message_stream::WindowInputDispatchResult::Camera);
    }
    let ui_operation = matches!(
        request,
        game_engine::common::message_stream::WindowInputDispatch::Ui(_)
    );
    let game_ending = context.game_ending;
    let result = owner.try_with_manager(|manager| match request {
        game_engine::common::message_stream::WindowInputDispatch::OpaqueWindowAtScreen {
            ..
        }
        | game_engine::common::message_stream::WindowInputDispatch::Display(_)
        | game_engine::common::message_stream::WindowInputDispatch::Camera(_) => {
            unreachable!("display dispatch completed above")
        }
        game_engine::common::message_stream::WindowInputDispatch::Ui(action) => {
            let handled = context.with_effects(manager, |manager, effects| {
                crate::message_stream::meta_event::execute_ui_input_action(
                    action,
                    manager,
                    effects,
                    game_ending,
                )
            });
            if handled {
                WindowInputReturnCode::Used
            } else {
                WindowInputReturnCode::NotUsed
            }
        }
        game_engine::common::message_stream::WindowInputDispatch::Mouse {
            message,
            x,
            y,
            data,
            delta,
        } => manager.process_mouse_event_with_delta_and_control_bar(
            WindowMessage::from(message),
            x,
            y,
            data as WindowMsgData,
            delta,
            context,
        ),
        game_engine::common::message_stream::WindowInputDispatch::Key { key, state } => {
            manager.process_key_event_with_control_bar(key, state, context)
        }
    });
    {
        let messages = context
            .emitted_messages
            .as_deref_mut()
            .expect("native translator requires receiving stream");
        owner.with_manager(|manager| {
            context.control_bar.replay_pending_command_clicks_receiving(
                owner,
                messages,
                manager,
                context.ui.as_deref_mut().expect("receiving UI"),
            )
        });
    }
    owner.with_manager(|manager| context.complete_options_accept(manager));
    {
        let mut effects = crate::gui::game_window::WindowEventEffects {
            input_provenance: context.input_provenance,
            standalone_view: None,
            ui: context.ui.as_deref_mut(),
            logic_frame: context.logic_frame,
            client_frame: context.client_frame,
            payloads: Default::default(),
            diplomacy_world: None,
            menus: context.menus.as_mut().map(|menus| menus.reborrow()),
            ime: context.ime.as_deref_mut(),
            manager: None,
            window_manager_owner: Some(owner),
            selected_window: None,
            emitted_messages: context
                .emitted_messages
                .as_deref_mut()
                .expect("native translator requires receiving stream"),
            pending_control_bar_clicks: Some(&mut context.control_bar.pending_command_clicks),
            campaign_manager: context.campaign_manager.as_deref_mut(),
            main_menu: context.main_menu.as_deref_mut(),
            menu_world: context.menu_world,
            menu_host: context.menu_host.as_deref_mut(),
            script_display: context.script_display.as_deref_mut(),
            window_videos: context.window_videos.as_deref_mut(),
            pending_main_menu_actions: Vec::new(),
        };
        owner.complete_event_callbacks(&mut effects);
    }
    owner.with_manager(|manager| {
        context.control_bar.replay_pending_command_clicks_receiving(
            owner,
            context
                .emitted_messages
                .as_deref_mut()
                .expect("native input requires receiving stream"),
            manager,
            context.ui.as_deref_mut().expect("receiving UI"),
        )
    });
    result.map(|code| {
        if ui_operation {
            return game_engine::common::message_stream::WindowInputDispatchResult::Ui(
                code == WindowInputReturnCode::Used,
            );
        }

        if code == WindowInputReturnCode::Used {
            game_engine::common::message_stream::WindowInputDispatchResult::Used
        } else {
            game_engine::common::message_stream::WindowInputDispatchResult::NotUsed
        }
    })
}

/// Native mouse dispatch with Main's receiving ControlBar context.
pub fn dispatch_os_mouse_to_window_manager_with_control_bar_for_owner(
    owner: &WindowManagerOwner,
    msg: WindowMessage,
    x: i32,
    y: i32,
    facts: WindowMouseFacts,
    context: &mut super::ControlBarInputContext<'_>,
) -> WindowInputReturnCode {
    let result = dispatch_mouse_with_facts(
        msg,
        x,
        y,
        facts,
        |msg, x, y, data| {
            owner.with_manager(|manager| {
                manager.process_mouse_event_with_control_bar(msg, x, y, data, context)
            })
        },
        || {
            owner
                .with_shell_ref(Shell::is_shell_active)
                .unwrap_or(false)
        },
    );
    if let Some(messages) = context.emitted_messages.as_deref_mut() {
        owner.with_manager(|manager| {
            context.control_bar.replay_pending_command_clicks_receiving(
                owner,
                messages,
                manager,
                context.ui.as_deref_mut().expect("receiving UI"),
            )
        });
    } else {
        assert!(
            context.control_bar.pending_command_clicks.is_empty(),
            "native command input requires receiving stream"
        );
    }
    owner.with_manager(|manager| context.complete_options_accept(manager));
    result
}

/// Native receiving route. The manager callback/re-entry and shell policy are
/// the same as ordinary Core dispatch; only camera fact discovery differs.

/// Native selected WND actions receive the actual bar and view for this call.

fn dispatch_mouse_with_facts(
    msg: WindowMessage,
    x: i32,
    y: i32,
    facts: WindowMouseFacts,
    dispatch: impl FnOnce(WindowMessage, i32, i32, WindowMsgData) -> WindowInputReturnCode,
    shell_active: impl FnOnce() -> bool,
) -> WindowInputReturnCode {
    if facts.mouse_locked
        && crate::message_stream::window_xlat::should_skip_os_window_manager_while_mouse_locked(
            msg,
            facts.scrolling,
        )
    {
        return WindowInputReturnCode::NotUsed;
    }
    let data = (((y as u32) << 16) | ((x as u32) & 0xFFFF)) as WindowMsgData;
    let rc = dispatch(msg, x, y, data);
    if shell_active() {
        WindowInputReturnCode::Used
    } else {
        rc
    }
}

#[cfg(test)]
#[path = "mouse_facts_tests.rs"]
mod mouse_facts_tests;

/// OS key → focused window `GWM_CHAR` (C++ `WindowXlat` RAW_KEY_DOWN/UP).
///
/// `state` is the C++ key-state byte (`KEY_STATE_DOWN=0x02`, `KEY_STATE_UP=0x01`).
/// Shell active consumes the key so world hotkeys do not also fire.

/// Dispatch a native keyboard event to the explicitly receiving client's WND
/// tree. Focus and callback order belong to that owner.
pub fn dispatch_os_key_to_window_manager_for_owner(
    owner: &WindowManagerOwner,
    key: u8,
    state: u8,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowInputReturnCode {
    // C++ WindowXlat.cpp RAW_KEY_DOWN/UP consumes only the focused WND result
    // (plus its separate Escape fallbacks); unlike mouse input, Shell-active
    // alone does not consume an otherwise-unhandled key.
    owner.with_manager(|manager| manager.process_key_event(key, state, effects))
}

/// Native keyboard dispatch with Main's receiving ControlBar context.
pub fn dispatch_os_key_to_window_manager_with_control_bar_for_owner(
    owner: &WindowManagerOwner,
    key: u8,
    state: u8,
    context: &mut super::ControlBarInputContext<'_>,
) -> WindowInputReturnCode {
    let result = owner
        .with_manager(|manager| manager.process_key_event_with_control_bar(key, state, context));
    if let Some(messages) = context.emitted_messages.as_deref_mut() {
        owner.with_manager(|manager| {
            context.control_bar.replay_pending_command_clicks_receiving(
                owner,
                messages,
                manager,
                context.ui.as_deref_mut().expect("receiving UI"),
            )
        });
    } else {
        assert!(
            context.control_bar.pending_command_clicks.is_empty(),
            "native command input requires receiving stream"
        );
    }
    owner.with_manager(|manager| context.complete_options_accept(manager));
    result
}

pub(crate) fn apply_draw_callback_override(
    manager: &mut WindowManager,
    window_name: &str,
    draw: fn(
        &GameWindow,
        &WindowInstanceData,
        &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
    ),
) {
    if let Some(window) = manager.find_window_by_name(window_name) {
        window.borrow_mut().set_draw_callback(draw);
    }
}

pub(crate) fn apply_w3d_main_menu_runtime_draw_overrides(manager: &mut WindowManager) {
    for window_name in [
        "MainMenu.wnd:ButtonSkirmish",
        "MainMenu.wnd:ButtonOnline",
        "MainMenu.wnd:ButtonNetwork",
        "MainMenu.wnd:ButtonUSA",
        "MainMenu.wnd:ButtonGLA",
        "MainMenu.wnd:ButtonChina",
        "MainMenu.wnd:ButtonMultiBack",
        "MainMenu.wnd:ButtonSingleBack",
        "MainMenu.wnd:ButtonExit",
        "MainMenu.wnd:ButtonOptions",
        "MainMenu.wnd:ButtonMultiplayer",
        "MainMenu.wnd:ButtonSinglePlayer",
        "MainMenu.wnd:ButtonReplay",
        "MainMenu.wnd:ButtonLoadGame",
        "MainMenu.wnd:ButtonLoadReplay",
        "MainMenu.wnd:ButtonLoadReplayBack",
        "MainMenu.wnd:ButtonTRAINING",
        "MainMenu.wnd:ButtonCredits",
    ] {
        apply_draw_callback_override(manager, window_name, w3d_main_menu_button_drop_shadow_draw);
    }

    for window_name in [
        "MainMenu.wnd:StaticTextRandom1",
        "MainMenu.wnd:StaticTextRandom2",
    ] {
        apply_draw_callback_override(manager, window_name, w3d_main_menu_random_text_draw);
    }
}

#[cfg(test)]
mod owner_tests {
    use super::WindowManagerOwner;
    use crate::system::SubsystemInterface as _;
    use std::rc::Rc;

    #[test]
    fn receiving_owner_keeps_same_id_windows_in_distinct_client_trees() {
        let first = WindowManagerOwner::new();
        let second = WindowManagerOwner::new();
        let first_window =
            first.with_manager(|manager| manager.create_window(None, 0, 0, 20, 20).unwrap());
        let second_window =
            second.with_manager(|manager| manager.create_window(None, 0, 0, 20, 20).unwrap());
        assert_eq!(first_window.borrow().id, second_window.borrow().id);
        assert!(!Rc::ptr_eq(&first_window, &second_window));
        first.with_manager(|manager| {
            manager.destroy_window(first_window.clone()).unwrap();
            manager.flush_destroy_queue();
            assert!(manager.get_window_by_id(first_window.borrow().id).is_none());
        });
        second.with_manager_ref(|manager| {
            assert!(Rc::ptr_eq(
                &manager.get_window_by_id(second_window.borrow().id).unwrap(),
                &second_window
            ))
        });
    }

    #[test]
    fn nested_explicit_owner_borrows_never_select_a_peer() {
        let first = WindowManagerOwner::new();
        let second = WindowManagerOwner::new();
        first.with_manager(|manager| {
            let window = manager.create_window(None, 0, 0, 20, 20).unwrap();
            second.with_manager(|peer| {
                peer.create_window(None, 0, 0, 20, 20).unwrap();
            });
            assert!(
                manager
                    .get_window_by_id(window.borrow().id)
                    .is_some_and(|found| Rc::ptr_eq(&found, &window))
            );
        });
    }

    #[test]
    fn shell_stack_and_late_owner_handle_survive_equal_id_peer_reset() {
        if crate::core::game_client::global_boundary_fixture::run_in_private_child(
            module_path!(),
            "shell_stack_and_late_owner_handle_survive_equal_id_peer_reset",
        ) {
            return;
        }
        let mut first = crate::core::game_client::GameClient::new().unwrap();
        let mut second = crate::core::game_client::GameClient::new().unwrap();
        let first_owner = first.window_manager_owner();
        let second_owner = second.window_manager_owner();
        let first_window =
            first_owner.with_manager(|manager| manager.create_window(None, 0, 0, 20, 20).unwrap());
        let second_window =
            second_owner.with_manager(|manager| manager.create_window(None, 0, 0, 20, 20).unwrap());
        assert_eq!(first_window.borrow().id, second_window.borrow().id);
        assert!(!Rc::ptr_eq(&first_window, &second_window));
        first
            .with_receiving_shell(None, |shell, effects| {
                shell.init().unwrap();
                shell.push("Menus/MainMenu.wnd", false, effects).unwrap();
            })
            .unwrap();
        second
            .with_receiving_shell(None, |shell, effects| {
                shell.init().unwrap();
                shell.push("Menus/MainMenu.wnd", false, effects).unwrap();
            })
            .unwrap();
        assert_eq!(
            first_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(1)
        );
        assert_eq!(
            second_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(1)
        );
        let late_first_owner = first_owner.clone();
        second
            .with_receiving_shell(None, |shell, effects| {
                shell.reset_receiving(effects).unwrap()
            })
            .unwrap();
        assert_eq!(
            second_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(0)
        );
        assert_eq!(
            late_first_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(1)
        );
        assert!(first_owner.with_manager_ref(|manager| {
            manager.get_window_by_id(first_window.borrow().id).is_some()
        }));
        assert!(second_owner.with_manager_ref(|manager| {
            manager
                .get_window_by_id(second_window.borrow().id)
                .is_some()
        }));
        late_first_owner.queue_shell_operation_with_effects(|shell, effects| {
            assert_eq!(shell.top_filename(), Some("Menus/MainMenu.wnd"));
            shell.pop(effects).unwrap();
        });
        first.with_window_event_effects(None, |_| {});
        assert_eq!(
            late_first_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(1)
        );
        assert_eq!(
            late_first_owner.with_shell_ref(|shell| shell.top_filename().map(str::to_owned)),
            Some(Some("Menus/MainMenu.wnd".into()))
        );
        first
            .with_receiving_shell(None, |shell, effects| {
                shell.shutdown_complete(None, false, effects).unwrap()
            })
            .unwrap();
        assert_eq!(
            late_first_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(0)
        );
        first
            .with_receiving_shell(None, |shell, effects| {
                shell.push("Menus/MainMenu.wnd", false, effects).unwrap()
            })
            .unwrap();
        assert_eq!(
            late_first_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(1)
        );
        assert_eq!(
            second_owner.with_shell_ref(|shell| shell.get_screen_count()),
            Some(0)
        );
    }

    #[test]
    fn shell_residuals_and_deferred_pushes_are_receiving_owner_state() {
        use crate::gui::shell::ResidualShellMapAction;
        let first = WindowManagerOwner::new();
        let second = WindowManagerOwner::new();
        assert!(first.queue_deferred_shell_push("Menus/MainMenu.wnd"));
        assert!(!first.queue_deferred_shell_push("Menus/MainMenu.wnd"));
        assert!(second.queue_deferred_shell_push("Menus/OptionsMenu.wnd"));
        assert_eq!(
            first.drain_deferred_shell_pushes(),
            vec!["Menus/MainMenu.wnd".to_string()]
        );
        assert_eq!(second.clear_deferred_shell_pushes(), 1);
        assert!(second.drain_deferred_shell_pushes().is_empty());
        first.set_residual_shell_map(ResidualShellMapAction::Show, true);
        second.set_residual_shell_map(ResidualShellMapAction::Hide, false);
        assert!(first.residual_shell_map_on());
        assert!(!second.residual_shell_map_on());
        assert_eq!(
            first.residual_shell_map_action(),
            ResidualShellMapAction::Show
        );
        assert_eq!(
            second.residual_shell_map_action(),
            ResidualShellMapAction::Hide
        );
    }

    #[test]
    fn owner_borrow_unwinds_and_keeps_callback_queues_separate() {
        use std::cell::RefCell;
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let mut first = crate::core::game_client::GameClient::new().unwrap();
        let mut second = crate::core::game_client::GameClient::new().unwrap();
        let first_owner = first.window_manager_owner();
        let second_owner = second.window_manager_owner();
        let first_log = Rc::new(RefCell::new(Vec::new()));
        let second_log = Rc::new(RefCell::new(Vec::new()));
        let result = Rc::clone(&first_log);
        first_owner
            .queue_window_operation_with_effects(move |_, _| result.borrow_mut().push("first"));
        first.with_window_event_effects(None, |_| {});
        let result = Rc::clone(&second_log);
        second_owner
            .queue_window_operation_with_effects(move |_, _| result.borrow_mut().push("second"));
        second.with_window_event_effects(None, |_| {});
        assert_eq!(&*first_log.borrow(), &["first"]);
        assert_eq!(&*second_log.borrow(), &["second"]);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            first_owner.with_manager(|_| {
                second_owner.with_manager(|_| panic!("exercise actual owner borrow unwind"))
            })
        }));
        first_owner.with_manager(|_| first_log.borrow_mut().push("after unwind"));
        assert_eq!(&*first_log.borrow(), &["first", "after unwind"]);
    }
}
