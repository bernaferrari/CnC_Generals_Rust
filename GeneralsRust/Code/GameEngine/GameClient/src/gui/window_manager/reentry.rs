//! Singleton re-entry queue, typed fail-closed payloads, and OS dispatch.
#![allow(unused_imports)]

use crate::gui::game_window::*;
use crate::gui::load_screen::{LoadScreenPreludeOutcome, LoadScreenPreludeStep};
use crate::gui::shell::get_shell;
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

thread_local! {
    static THE_WINDOW_MANAGER: RefCell<WindowManager> = RefCell::new(WindowManager::new());
    /// Side-effecting ops queued via [`queue_window_manager_op`] when the singleton
    /// is already borrowed. Flushed by the stack owner after the outer
    /// `&mut WindowManager` is the only live mut ref.
    static WINDOW_MANAGER_OP_QUEUE: RefCell<Vec<Box<dyn FnOnce(&mut WindowManager) + 'static>>> =
        RefCell::new(Vec::new());
    /// Ops that could not mutate a live window RefCell during drain. Spliced
    /// into the regular queue at the next `with_window_manager` entry so we
    /// do not spin the drain loop while the cell is still borrowed.
    static WINDOW_MANAGER_DEFERRED_OPS: RefCell<Vec<Box<dyn FnOnce(&mut WindowManager) + 'static>>> =
        RefCell::new(Vec::new());
    /// Empty snapshot used when a shared read re-enters while a mutable borrow is live.
    /// Draw helpers (`win_font_height`, `win_draw_image`, …) ignore `self`; lookups fail-closed.
    static WINDOW_MANAGER_FAIL_CLOSED: WindowManager = WindowManager::new();
}

fn drain_window_manager_ops(manager: &mut WindowManager) {
    loop {
        let ops = WINDOW_MANAGER_OP_QUEUE.with(|queue| queue.replace(Vec::new()));
        if ops.is_empty() {
            break;
        }
        for op in ops {
            op(manager);
        }
    }
}

fn splice_deferred_window_manager_ops() {
    let deferred = WINDOW_MANAGER_DEFERRED_OPS.with(|q| q.replace(Vec::new()));
    if deferred.is_empty() {
        return;
    }
    WINDOW_MANAGER_OP_QUEUE.with(|queue| queue.borrow_mut().extend(deferred));
}

/// Queue work for the *next* `with_window_manager` entry, not the in-flight drain.
/// Used when a window RefCell is still borrowed during drain.
pub fn queue_window_manager_op_deferred(f: impl FnOnce(&mut WindowManager) + 'static) {
    WINDOW_MANAGER_DEFERRED_OPS.with(|queue| queue.borrow_mut().push(Box::new(f)));
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
/// Runs immediately when the singleton is free; otherwise runs when the outer
/// `with_window_manager` / input dispatch drains the queue.
pub fn queue_window_manager_op(f: impl FnOnce(&mut WindowManager) + 'static) {
    THE_WINDOW_MANAGER.with(|manager| {
        if let Ok(mut borrow) = manager.try_borrow_mut() {
            f(&mut borrow);
            drain_window_manager_ops(&mut borrow);
            return;
        }
        WINDOW_MANAGER_OP_QUEUE.with(|queue| queue.borrow_mut().push(Box::new(f)));
    });
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
    THE_WINDOW_MANAGER.with(|manager| match manager.try_borrow_mut() {
        Ok(mut borrow) => {
            splice_deferred_window_manager_ops();
            drain_window_manager_ops(&mut borrow);
            let result = f(&mut borrow);
            drain_window_manager_ops(&mut borrow);
            result
        }
        Err(_) => window_manager_reentry(f),
    })
}

pub fn with_window_manager_ref<R>(f: impl FnOnce(&WindowManager) -> R) -> R {
    THE_WINDOW_MANAGER.with(|manager| match manager.try_borrow() {
        Ok(borrow) => f(&borrow),
        Err(_) => {
            // Mutable borrow is live. Do not alias `&WindowManager` with that `&mut`.
            // Fail-closed: empty snapshot. Font/image draw helpers ignore `self`.
            WINDOW_MANAGER_FAIL_CLOSED.with(|dummy| f(dummy))
        }
    })
}

/// True when the thread-local WindowManager is not currently mutably borrowed.
/// Used by MainMenuInit to skip hide/bring-forward work that would stall while
/// Shell::push still holds the manager.
pub fn window_manager_try_borrow_free() -> bool {
    THE_WINDOW_MANAGER.with(|manager| manager.try_borrow_mut().is_ok())
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
pub fn dispatch_os_mouse_to_window_manager(
    msg: WindowMessage,
    x: i32,
    y: i32,
) -> WindowInputReturnCode {
    if crate::message_stream::window_xlat::os_mouse_blocked_by_mouse_lock(msg) {
        return WindowInputReturnCode::NotUsed;
    }
    let data = (((y as u32) << 16) | ((x as u32) & 0xFFFF)) as WindowMsgData;
    let rc = with_window_manager(|manager| manager.process_mouse_event(msg, x, y, data));
    if crate::gui::shell::get_shell().is_shell_active() {
        WindowInputReturnCode::Used
    } else {
        rc
    }
}

/// OS key → focused window `GWM_CHAR` (C++ `WindowXlat` RAW_KEY_DOWN/UP).
///
/// `state` is the C++ key-state byte (`KEY_STATE_DOWN=0x02`, `KEY_STATE_UP=0x01`).
/// Shell active consumes the key so world hotkeys do not also fire.
pub fn dispatch_os_key_to_window_manager(key: u8, state: u8) -> WindowInputReturnCode {
    let rc = with_window_manager(|manager| manager.process_key_event(key, state));
    if crate::gui::shell::get_shell().is_shell_active() {
        WindowInputReturnCode::Used
    } else {
        rc
    }
}

pub(crate) fn apply_draw_callback_override(
    window_name: &str,
    draw: fn(&GameWindow, &WindowInstanceData),
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
