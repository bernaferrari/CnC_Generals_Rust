//! Owner-scoped re-entry queues, typed fail-closed payloads, and OS dispatch.
#![allow(unused_imports)]

use crate::gui::game_window::*;
use crate::gui::load_screen::{LoadScreenPreludeOutcome, LoadScreenPreludeStep};
use crate::gui::shell::{ResidualShellMapAction, Shell, get_shell};
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
    Legacy(Box<dyn FnOnce(&mut WindowManager) + 'static>),
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
    Legacy(Box<dyn FnOnce(&mut Shell) + 'static>),
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
    deferred_operations: RefCell<Vec<WindowManagerOperation>>,
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

impl WindowManagerOwner {
    pub fn new() -> Self {
        Self {
            state: Rc::new(WindowManagerOwnerState {
                manager: RefCell::new(WindowManager::new()),
                operations: RefCell::new(Vec::new()),
                deferred_operations: RefCell::new(Vec::new()),
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
        self.state.deferred_operations.borrow_mut().clear();
        self.state.shell_operations.borrow_mut().clear();
        self.state.deferred_shell_pushes.borrow_mut().clear();
    }

    /// Run one synchronous operation with this owner selected for legacy nested
    /// callbacks. The selector is restored on return and unwind and cannot escape.
    pub fn with_scope<R>(&self, operation: impl FnOnce() -> R) -> R {
        let _scope = self.enter_scoped();
        operation()
    }

    fn enter_scoped(&self) -> WindowManagerScope {
        let previous = ACTIVE_WINDOW_MANAGER_OWNER
            .with(|current| current.replace(Some(Rc::clone(&self.state))));
        WindowManagerScope { previous }
    }

    /// Borrow this exact owner's manager for one synchronous operation.
    pub fn with_manager<R: ReentryFallback>(&self, f: impl FnOnce(&mut WindowManager) -> R) -> R {
        let owner = Rc::clone(&self.state);
        self.with_scope(|| match owner.manager.try_borrow_mut() {
            Ok(mut borrow) => {
                splice_deferred_window_manager_ops(&owner);
                drain_window_manager_ops(&owner, &mut borrow);
                let result = f(&mut borrow);
                drain_window_manager_ops(&owner, &mut borrow);
                result
            }
            Err(_) => window_manager_reentry(f),
        })
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
        let owner = &self.state;
        self.with_scope(|| {
            let mut manager = owner.manager.try_borrow_mut().ok()?;
            splice_deferred_window_manager_ops(owner);
            drain_window_manager_ops(owner, &mut manager);
            let result = f(&mut manager);
            drain_window_manager_ops(owner, &mut manager);
            Some(result)
        })
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
        let owner = Rc::clone(&self.state);
        self.with_scope(|| {
            let mut manager = owner
                .manager
                .try_borrow_mut()
                .expect("WND callback replay requires outer manager loan released");
            drain_window_manager_ops_with_effects(&owner, &mut manager, effects);
        });
    }

    /// Read this exact owner's manager for one synchronous operation.
    pub fn with_manager_ref<R: ReentryFallback>(&self, f: impl FnOnce(&WindowManager) -> R) -> R {
        let owner = Rc::clone(&self.state);
        self.with_scope(|| match owner.manager.try_borrow() {
            Ok(borrow) => f(&borrow),
            Err(_) => {
                // A live mutable borrow must not be aliased with a shared one.
                let inert = WindowManager::new();
                f(&inert)
            }
        })
    }

    /// Mutate this client's Shell for one synchronous lifecycle operation.
    /// Nested callback work is drained before the sole mutable borrow ends.
    pub fn with_shell_mut<R>(&self, f: impl FnOnce(&mut Shell) -> R) -> Option<R> {
        let owner = Rc::clone(&self.state);
        self.with_scope(|| {
            let mut shell = owner.shell.try_borrow_mut().ok()?;
            drain_shell_operations(&owner, &mut shell);
            if !owner.shell_operations.borrow().is_empty() {
                return None;
            }
            let mut drain = ShellOperationDrain {
                owner: Rc::clone(&owner),
                shell: &mut shell,
            };
            let result = f(drain.shell);
            drop(drain);
            Some(result)
        })
    }

    /// Complete the receiving client's queued lifecycle effects in their original event phase.
    pub(crate) fn with_shell_effects_mut<R>(
        &self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
        f: impl FnOnce(&mut Shell, &mut crate::gui::game_window::WindowEventEffects<'_>) -> R,
    ) -> Option<R> {
        let owner = Rc::clone(&self.state);
        self.with_scope(|| {
            let mut shell = owner.shell.try_borrow_mut().ok()?;
            drain_shell_operations_with_effects(&owner, &mut shell, effects);
            let result = f(&mut shell, effects);
            drain_shell_operations_with_effects(&owner, &mut shell, effects);
            Some(result)
        })
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
        let owner = Rc::clone(&self.state);
        self.with_scope(|| owner.shell.try_borrow().ok().map(|shell| f(&shell)))
    }

    /// Run or queue work on this owner's Shell, preserving the same re-entry boundary.
    pub fn queue_shell_operation(&self, operation: impl FnOnce(&mut Shell) + 'static) {
        let owner = Rc::clone(&self.state);
        if !owner.shell_operations.borrow().is_empty() {
            owner
                .shell_operations
                .borrow_mut()
                .push_back(ShellOperation::Legacy(Box::new(operation)));
            return;
        }
        self.with_scope(|| match owner.shell.try_borrow_mut() {
            Ok(mut shell) => {
                drain_shell_operations(&owner, &mut shell);
                operation(&mut shell);
                drain_shell_operations(&owner, &mut shell);
            }
            Err(_) => owner
                .shell_operations
                .borrow_mut()
                .push_back(ShellOperation::Legacy(Box::new(operation))),
        });
    }

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

fn drain_shell_operations(owner: &Rc<WindowManagerOwnerState>, shell: &mut Shell) {
    loop {
        let operation = {
            let mut operations = owner.shell_operations.borrow_mut();
            if !matches!(operations.front(), Some(ShellOperation::Legacy(_))) {
                return;
            }
            operations.pop_front().unwrap()
        };
        if let ShellOperation::Legacy(operation) = operation {
            operation(shell);
        }
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
            Some(ShellOperation::Legacy(operation)) => operation(shell),
            Some(ShellOperation::WithEffects(operation)) => operation(shell, effects),
            None => return,
        }
    }
}

struct RestoreShellOperationsOnUnwind<'a> {
    owner: &'a WindowManagerOwnerState,
    pending: &'a mut VecDeque<ShellOperation>,
    armed: bool,
}

impl Drop for RestoreShellOperationsOnUnwind<'_> {
    fn drop(&mut self) {
        if self.armed && std::thread::panicking() && !self.pending.is_empty() {
            let mut remaining = std::mem::take(self.pending);
            let mut queued = self.owner.shell_operations.borrow_mut();
            remaining.append(&mut queued);
            *queued = remaining;
        }
    }
}

struct ShellOperationDrain<'a> {
    owner: Rc<WindowManagerOwnerState>,
    shell: &'a mut Shell,
}

impl Drop for ShellOperationDrain<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                drain_shell_operations(&self.owner, self.shell);
            }));
            if result.is_err() {
                log::error!("queued shell operation panicked while a layout callback unwound");
            }
        } else {
            drain_shell_operations(&self.owner, self.shell);
        }
    }
}

pub(crate) fn current_window_manager_owner() -> Option<WindowManagerOwner> {
    active_window_manager_owner().map(|state| WindowManagerOwner { state })
}

/// Restores the prior explicit manager selection at the end of a synchronous
/// native UI operation. It contains no pointer and does not outlive the stack.
struct WindowManagerScope {
    previous: Option<Rc<WindowManagerOwnerState>>,
}

impl Drop for WindowManagerScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE_WINDOW_MANAGER_OWNER.with(|current| {
            current.replace(previous);
        });
    }
}

thread_local! {
    /// Scoped compatibility context only. Production callers establish it from
    /// their persistent GameClient owner; it never owns a fallback WND tree.
    static ACTIVE_WINDOW_MANAGER_OWNER: RefCell<Option<Rc<WindowManagerOwnerState>>> =
        const { RefCell::new(None) };
}

fn active_window_manager_owner() -> Option<Rc<WindowManagerOwnerState>> {
    ACTIVE_WINDOW_MANAGER_OWNER.with(|current| current.borrow().clone())
}

fn drain_window_manager_ops(owner: &WindowManagerOwnerState, manager: &mut WindowManager) {
    loop {
        let op = {
            let mut operations = owner.operations.borrow_mut();
            if !matches!(operations.first(), Some(WindowManagerOperation::Legacy(_))) {
                return;
            }
            operations.remove(0)
        };
        if let WindowManagerOperation::Legacy(operation) = op {
            operation(manager);
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
            WindowManagerOperation::Legacy(operation) => operation(manager),
            WindowManagerOperation::WithEffects(operation) => operation(manager, effects),
        }
    }
}

fn splice_deferred_window_manager_ops(owner: &WindowManagerOwnerState) {
    let deferred = owner.deferred_operations.replace(Vec::new());
    if deferred.is_empty() {
        return;
    }
    owner.operations.borrow_mut().extend(deferred);
}

/// Queue work for the *next* `with_window_manager` entry, not the in-flight drain.
/// Used when a window RefCell is still borrowed during drain.
pub fn queue_window_manager_op_deferred(f: impl FnOnce(&mut WindowManager) + 'static) {
    if let Some(owner) = active_window_manager_owner() {
        owner
            .deferred_operations
            .borrow_mut()
            .push(WindowManagerOperation::Legacy(Box::new(f)));
    }
}

/// Hide/show without panicking if the window RefCell is already borrowed.
/// Re-queues until the cell is free (next outer drain), not fail-closed no-op.
pub fn hide_window_rc(win_rc: &Rc<RefCell<GameWindow>>, hide: bool) {
    match win_rc.try_borrow_mut() {
        Ok(mut win) => {
            let _ = win.hide(hide);
        }
        Err(_) => {
            let win_rc = win_rc.clone();
            queue_window_manager_op_deferred(move |_manager| {
                if let Ok(mut win) = win_rc.try_borrow_mut() {
                    let _ = win.hide(hide);
                }
            });
        }
    }
}

/// Queue a `'static` window-manager side effect.
///
/// Runs immediately when the receiving owner is free; otherwise runs when the outer
/// `with_window_manager` / input dispatch drains that owner’s queue.
pub fn queue_window_manager_op(f: impl FnOnce(&mut WindowManager) + 'static) {
    let Some(owner) = active_window_manager_owner() else {
        return;
    };
    {
        if !owner.operations.borrow().is_empty() {
            owner
                .operations
                .borrow_mut()
                .push(WindowManagerOperation::Legacy(Box::new(f)));
            return;
        }
        if let Ok(mut borrow) = owner.manager.try_borrow_mut() {
            f(&mut borrow);
            drain_window_manager_ops(&owner, &mut borrow);
            return;
        }
        owner
            .operations
            .borrow_mut()
            .push(WindowManagerOperation::Legacy(Box::new(f)));
    }
}

/// Nested Start / hide-parent create must queue, not fail-closed no-op.
pub fn queue_create_layout(filename: impl Into<String>) {
    let filename = filename.into();
    queue_window_manager_op(move |manager| {
        let _ = manager.create_layout_with_windows(&filename);
    });
}

/// Nested focus during a gadget callback must queue, not fail-closed no-op.
pub fn queue_set_focus(window: Rc<RefCell<GameWindow>>) {
    queue_window_manager_op(move |manager| {
        let _ = manager.set_focus(Some(&window));
    });
}

/// Access `TheWindowManager` mutably.
///
/// On re-entry (RefCell already mutably borrowed) this does **not** create an overlapping
/// `&mut WindowManager` via `as_ptr()`. Nested `f` is dropped unrun and the return
/// value is fail-closed (see [`window_manager_reentry_fallback`]), including `()`.
///
/// Side effects that must still run after the outer borrow go through
/// [`queue_window_manager_op`] with owned (`'static`) command data. That is the
/// working unit-reentry path: queue, then flush when the outer borrow ends.
pub fn with_window_manager<R: ReentryFallback>(f: impl FnOnce(&mut WindowManager) -> R) -> R {
    let Some(owner) = active_window_manager_owner() else {
        log::error!(
            "WindowManager access outside its receiving GameClient owner; using an inert fail-closed tree"
        );
        let mut inert = WindowManager::new();
        return f(&mut inert);
    };
    match owner.manager.try_borrow_mut() {
        Ok(mut borrow) => {
            splice_deferred_window_manager_ops(&owner);
            drain_window_manager_ops(&owner, &mut borrow);
            let result = f(&mut borrow);
            drain_window_manager_ops(&owner, &mut borrow);
            result
        }
        Err(_) => window_manager_reentry(f),
    }
}

pub fn with_window_manager_ref<R>(f: impl FnOnce(&WindowManager) -> R) -> R {
    let Some(owner) = active_window_manager_owner() else {
        log::error!(
            "WindowManager read outside its receiving GameClient owner; using an inert fail-closed tree"
        );
        let inert = WindowManager::new();
        return f(&inert);
    };
    match owner.manager.try_borrow() {
        Ok(borrow) => f(&borrow),
        Err(_) => {
            // Mutable borrow is live. Do not alias `&WindowManager` with that `&mut`.
            // Fail-closed: an empty snapshot. Font/image draw helpers ignore `self`.
            let inert = WindowManager::new();
            f(&inert)
        }
    }
}

/// True when the active owner’s WindowManager is not currently mutably borrowed.
/// Used by MainMenuInit to skip hide/bring-forward work that would stall while
/// Shell::push still holds the manager.
pub fn window_manager_try_borrow_free() -> bool {
    active_window_manager_owner().is_some_and(|owner| owner.manager.try_borrow_mut().is_ok())
}

fn window_manager_reentry<R: ReentryFallback>(f: impl FnOnce(&mut WindowManager) -> R) -> R {
    // Do not transmute a borrowed closure into the TLS queue. Unit work that
    // must run after the outer borrow uses [`queue_window_manager_op`].
    drop(f);
    window_manager_reentry_fallback::<R>()
}

mod reentry_fallback_seal {
    pub trait Sealed {}
}

/// Typed fail-closed payload for re-entrant [`with_window_manager`].
///
/// Sealed to the known UI return types. There is no `transmute` path: only
/// types with an explicit impl can be constructed on re-entry.
pub trait ReentryFallback: Sized + 'static + reentry_fallback_seal::Sealed {
    fn fallback() -> Option<Self>;
}

macro_rules! impl_reentry_fallback {
    ($($ty:ty => $value:expr),+ $(,)?) => {
        $(
            impl reentry_fallback_seal::Sealed for $ty {}
            impl ReentryFallback for $ty {
                fn fallback() -> Option<Self> {
                    Some($value)
                }
            }
        )+
    };
}

impl_reentry_fallback! {
    () => (),
    bool => false,
    i32 => 0,
    u32 => 0,
    usize => 0,
    (i32, i32) => (0, 0),
    (u32, u32) => (0, 0),
    WindowInputReturnCode => WindowInputReturnCode::NotUsed,
    Option<Rc<RefCell<GameWindow>>> => None,
    Option<(i32, i32)> => None,
    Option<bool> => None,
    Option<String> => None,
    Option<f32> => None,
    Option<WindowMsgHandled> => None,
    (WindowInputReturnCode, WindowInputReturnCode) => {
        (WindowInputReturnCode::NotUsed, WindowInputReturnCode::NotUsed)
    },
    Option<Rc<RefCell<WindowLayout>>> => None,
    Option<(Rc<RefCell<WindowLayout>>, WindowLayoutInfo)> => None,
    WindowResult<()> => Err(WindowError::GeneralFailure),
    WindowResult<Rc<RefCell<GameWindow>>> => Err(WindowError::GeneralFailure),
    WindowResult<WindowLayoutInfo> => Err(WindowError::GeneralFailure),
    WindowResult<(Rc<RefCell<WindowLayout>>, WindowLayoutInfo)> => {
        Err(WindowError::GeneralFailure)
    },
    // A nested campaign/Challenge prelude cannot safely borrow the WindowManager
    // again. Finish it fail-closed so the synchronous map-start caller neither
    // fabricates a frame advance nor spins while the outer UI callback owns it.
    LoadScreenPreludeStep => LoadScreenPreludeStep::Finished(LoadScreenPreludeOutcome::Skipped),
}

/// Known types that have no safe dummy. Re-entry panics rather than inventing
/// an `Rc` that is not in the live window tree.
macro_rules! impl_reentry_fallback_none {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl reentry_fallback_seal::Sealed for $ty {}
            impl ReentryFallback for $ty {
                fn fallback() -> Option<Self> {
                    None
                }
            }
        )+
    };
}

impl_reentry_fallback_none! {
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<WindowLayout>>,
    (Rc<RefCell<WindowLayout>>, WindowLayoutInfo),
}

/// Fail-closed values for re-entrant `with_window_manager`.
///
/// Documented defaults: `()`, `false`, `0`, `None`, `WindowInputReturnCode::NotUsed`,
/// `Err(WindowError::GeneralFailure)`. Types with no dummy (`Rc<…>`) panic so we
/// never invent a detached window/layout.
fn window_manager_reentry_fallback<R: ReentryFallback>() -> R {
    R::fallback().unwrap_or_else(|| {
        panic!(
            "re-entrant with_window_manager cannot fail-closed a value of type {}; \
             use queue_window_manager_op with owned command data",
            std::any::type_name::<R>()
        )
    })
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

pub fn dispatch_os_mouse_to_window_manager(
    msg: WindowMessage,
    x: i32,
    y: i32,
) -> WindowInputReturnCode {
    let mouse_locked = crate::display::view::with_tactical_view_ref(|view| view.is_mouse_locked());
    let facts = WindowMouseFacts {
        mouse_locked,
        scrolling: mouse_locked && crate::helpers::TheInGameUI::is_scrolling(),
    };
    dispatch_os_mouse_to_window_manager_with_facts(msg, x, y, facts)
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
) -> WindowInputReturnCode {
    dispatch_mouse_with_facts(
        msg,
        x,
        y,
        facts,
        |msg, x, y, data| {
            owner.with_manager(|manager| manager.process_mouse_event(msg, x, y, data))
        },
        || {
            owner
                .with_shell_ref(Shell::is_shell_active)
                .unwrap_or(false)
        },
    )
}

/// Registered WindowTranslator input uses the same receiving UI owners as physical input.
pub fn dispatch_translated_window_input_for_owner(
    owner: &WindowManagerOwner,
    request: game_engine::common::message_stream::WindowInputDispatch,
    context: &mut super::ControlBarInputContext<'_>,
) -> Option<game_engine::common::message_stream::WindowInputDispatchResult> {
    if let game_engine::common::message_stream::WindowInputDispatch::Display(action) = request {
        let display = context.script_display.as_deref_mut()?;
        return Some(
            game_engine::common::message_stream::WindowInputDispatchResult::Display(
                crate::message_stream::meta_event::execute_display_input_action(
                    action,
                    display,
                    owner,
                    context.window_videos.as_deref_mut()?,
                ),
            ),
        );
    }
    let result = owner.try_with_manager(|manager| match request {
        game_engine::common::message_stream::WindowInputDispatch::Display(_) => {
            unreachable!("display dispatch completed above")
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
        context
            .control_bar
            .replay_pending_command_clicks_receiving(owner, messages);
    }
    context.complete_options_accept();
    {
        let mut effects = crate::gui::game_window::WindowEventEffects {
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
    context.control_bar.replay_pending_command_clicks_receiving(
        owner,
        context.emitted_messages.as_deref_mut().unwrap(),
    );
    result.map(|code| {
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
        context
            .control_bar
            .replay_pending_command_clicks_receiving(owner, messages);
    } else {
        assert!(
            context.control_bar.pending_command_clicks.is_empty(),
            "native command input requires receiving stream"
        );
    }
    context.complete_options_accept();
    result
}

/// Native receiving route. The manager callback/re-entry and shell policy are
/// the same as ordinary Core dispatch; only camera fact discovery differs.
pub fn dispatch_os_mouse_to_window_manager_with_facts(
    msg: WindowMessage,
    x: i32,
    y: i32,
    facts: WindowMouseFacts,
) -> WindowInputReturnCode {
    dispatch_mouse_with_facts(
        msg,
        x,
        y,
        facts,
        |msg, x, y, data| {
            with_window_manager(|manager| manager.process_mouse_event(msg, x, y, data))
        },
        || crate::gui::shell::get_shell().is_shell_active(),
    )
}

/// Native selected WND actions receive the actual bar and view for this call.
pub fn dispatch_os_mouse_to_window_manager_with_control_bar(
    msg: WindowMessage,
    x: i32,
    y: i32,
    facts: WindowMouseFacts,
    context: &mut super::ControlBarInputContext<'_>,
) -> WindowInputReturnCode {
    dispatch_mouse_with_facts(
        msg,
        x,
        y,
        facts,
        |msg, x, y, data| {
            let handled = with_window_manager(|manager| {
                manager.process_mouse_event_with_control_bar(msg, x, y, data, context)
            });
            context.complete_options_accept();
            handled
        },
        || crate::gui::shell::get_shell().is_shell_active(),
    )
}

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
pub fn dispatch_os_key_to_window_manager_with_control_bar(
    key: u8,
    state: u8,
    context: &mut super::ControlBarInputContext<'_>,
) -> WindowInputReturnCode {
    let rc = with_window_manager(|manager| {
        manager.process_key_event_with_control_bar(key, state, context)
    });
    context.complete_options_accept();
    if crate::gui::shell::get_shell().is_shell_active() {
        WindowInputReturnCode::Used
    } else {
        rc
    }
}

pub fn dispatch_os_key_to_window_manager(key: u8, state: u8) -> WindowInputReturnCode {
    let rc = with_window_manager(|manager| manager.process_key_event(key, state));
    if crate::gui::shell::get_shell().is_shell_active() {
        WindowInputReturnCode::Used
    } else {
        rc
    }
}

/// Dispatch a native keyboard event to the explicitly receiving client's WND
/// tree. Focus and callback order belong to that owner.
pub fn dispatch_os_key_to_window_manager_for_owner(
    owner: &WindowManagerOwner,
    key: u8,
    state: u8,
) -> WindowInputReturnCode {
    // C++ WindowXlat.cpp RAW_KEY_DOWN/UP consumes only the focused WND result
    // (plus its separate Escape fallbacks); unlike mouse input, Shell-active
    // alone does not consume an otherwise-unhandled key.
    owner.with_manager(|manager| manager.process_key_event(key, state))
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
        context
            .control_bar
            .replay_pending_command_clicks_receiving(owner, messages);
    } else {
        assert!(
            context.control_bar.pending_command_clicks.is_empty(),
            "native command input requires receiving stream"
        );
    }
    context.complete_options_accept();
    result
}

pub(crate) fn apply_draw_callback_override(
    window_name: &str,
    draw: fn(
        &GameWindow,
        &WindowInstanceData,
        &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
    ),
) {
    with_window_manager(|manager| {
        if let Some(window) = manager.find_window_by_name(window_name) {
            window.borrow_mut().set_draw_callback(draw);
        }
    });
}

pub(crate) fn apply_w3d_main_menu_runtime_draw_overrides() {
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
        apply_draw_callback_override(window_name, w3d_main_menu_button_drop_shadow_draw);
    }

    for window_name in [
        "MainMenu.wnd:StaticTextRandom1",
        "MainMenu.wnd:StaticTextRandom2",
    ] {
        apply_draw_callback_override(window_name, w3d_main_menu_random_text_draw);
    }
}

#[cfg(test)]
mod owner_tests {
    use super::{
        WindowManagerOwner, queue_window_manager_op, queue_window_manager_op_deferred,
        with_window_manager,
    };
    use std::rc::Rc;

    #[test]
    fn scoped_owner_keeps_same_id_windows_in_distinct_client_trees() {
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
        second.with_manager(|manager| {
            assert!(Rc::ptr_eq(
                &manager.get_window_by_id(second_window.borrow().id).unwrap(),
                &second_window
            ));
        });
    }

    #[test]
    fn nested_explicit_owner_scope_restores_its_predecessor() {
        let first = WindowManagerOwner::new();
        let second = WindowManagerOwner::new();
        first.with_scope(|| {
            let first_window =
                with_window_manager(|manager| manager.create_window(None, 0, 0, 20, 20).unwrap());
            second.with_scope(|| {
                with_window_manager(|manager| {
                    manager.create_window(None, 0, 0, 20, 20).unwrap();
                });
            });
            assert!(with_window_manager(|manager| {
                manager
                    .get_window_by_id(first_window.borrow().id)
                    .is_some_and(|window| Rc::ptr_eq(&window, &first_window))
            }));
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
        let first = WindowManagerOwner::new();
        let second = WindowManagerOwner::new();
        let first_window =
            first.with_manager(|manager| manager.create_window(None, 0, 0, 20, 20).unwrap());
        let second_window =
            second.with_manager(|manager| manager.create_window(None, 0, 0, 20, 20).unwrap());
        assert_eq!(first_window.borrow().id, second_window.borrow().id);
        assert!(!Rc::ptr_eq(&first_window, &second_window));

        let mut first_shell = first.shell_handle();
        let mut second_shell = second.shell_handle();
        first_shell.init().unwrap();
        second_shell.init().unwrap();
        first_shell.push("Menus/MainMenu.wnd", false).unwrap();
        second_shell.push("Menus/MainMenu.wnd", false).unwrap();
        assert_eq!(first_shell.get_screen_count(), 1);
        assert_eq!(second_shell.get_screen_count(), 1);
        assert_eq!(
            first_shell.top_filename(),
            Some("Menus/MainMenu.wnd".to_string())
        );
        assert_eq!(
            second_shell.top_filename(),
            Some("Menus/MainMenu.wnd".to_string())
        );

        let mut late_first_shell = first.shell_handle();
        let late_callback_owner = first.clone();
        second_shell.reset().unwrap();
        assert_eq!(second_shell.get_screen_count(), 0);
        assert_eq!(late_first_shell.get_screen_count(), 1);
        assert!(first.with_manager_ref(|manager| {
            manager.get_window_by_id(first_window.borrow().id).is_some()
        }));
        assert!(second.with_manager_ref(|manager| {
            manager
                .get_window_by_id(second_window.borrow().id)
                .is_some()
        }));

        late_callback_owner.queue_shell_operation(|shell| {
            assert_eq!(shell.top_filename(), Some("Menus/MainMenu.wnd"));
            shell.pop().unwrap();
        });
        // C++ Shell::pop marks pendingPop and starts layout shutdown. The stack
        // entry remains until that layout reports shutdownComplete.
        assert_eq!(late_first_shell.get_screen_count(), 1);
        assert_eq!(
            late_first_shell.top_filename(),
            Some("Menus/MainMenu.wnd".to_string())
        );
        late_first_shell.shutdown_complete(None, false).unwrap();
        assert_eq!(late_first_shell.get_screen_count(), 0);
        late_first_shell.push("Menus/MainMenu.wnd", false).unwrap();
        assert_eq!(late_first_shell.get_screen_count(), 1);
        assert_eq!(second_shell.get_screen_count(), 0);
    }

    #[test]
    fn shell_residuals_and_deferred_pushes_are_receiving_owner_state() {
        use crate::gui::shell::{
            ResidualShellMapAction, residual_shell_map_is_on, residual_shell_map_last_action,
            simulate_shell_map_hide, simulate_shell_map_show,
        };
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

        assert!(first.with_scope(simulate_shell_map_show));
        assert!(second.with_scope(simulate_shell_map_hide));
        first.with_scope(|| {
            assert!(residual_shell_map_is_on());
            assert_eq!(
                residual_shell_map_last_action(),
                ResidualShellMapAction::Show
            );
        });
        second.with_scope(|| {
            assert!(!residual_shell_map_is_on());
            assert_eq!(
                residual_shell_map_last_action(),
                ResidualShellMapAction::Hide
            );
        });
    }

    #[test]
    fn owner_scope_restores_after_unwind_and_keeps_deferred_queues_separate() {
        use std::cell::{Cell, RefCell};
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let first = WindowManagerOwner::new();
        let second = WindowManagerOwner::new();
        let first_log = Rc::new(RefCell::new(Vec::new()));
        let second_log = Rc::new(RefCell::new(Vec::new()));
        let first_result = Rc::clone(&first_log);
        first.with_scope(|| {
            with_window_manager(|_| {
                queue_window_manager_op_deferred(move |_| first_result.borrow_mut().push("first"));
            });
            with_window_manager(|_| {});
            second.with_scope(|| {
                let second_result = Rc::clone(&second_log);
                with_window_manager(|_| {
                    queue_window_manager_op_deferred(move |_| {
                        second_result.borrow_mut().push("second")
                    });
                });
                with_window_manager(|_| {});
            });
        });
        assert_eq!(&*first_log.borrow(), &["first"]);
        assert_eq!(&*second_log.borrow(), &["second"]);

        let _ = catch_unwind(AssertUnwindSafe(|| {
            first.with_scope(|| {
                second.with_scope(|| panic!("exercise scoped owner unwind restoration"));
            });
        }));
        first.with_scope(|| {
            let actual = Rc::clone(&first_log);
            queue_window_manager_op(move |_| actual.borrow_mut().push("after unwind"));
        });
        assert_eq!(&*first_log.borrow(), &["first", "after unwind"]);
    }
}
