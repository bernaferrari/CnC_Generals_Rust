//! Singleton re-entry queue, typed fail-closed payloads, and OS dispatch.
#![allow(unused_imports)]

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use crate::gui::game_window::*;
use crate::gui::w3d_gadget_draw::{
    w3d_main_menu_button_drop_shadow_draw, w3d_main_menu_random_text_draw,
};
use crate::gui::shell::get_shell;

use super::*;

thread_local! {
    static THE_WINDOW_MANAGER: RefCell<WindowManager> = RefCell::new(WindowManager::new());
    /// Side-effecting `R = ()` ops enqueued when `with_window_manager` re-enters.
    /// Flushed by the stack owner after the outer `&mut WindowManager` is the only live mut ref.
    static WINDOW_MANAGER_OP_QUEUE: RefCell<Vec<Box<dyn FnOnce(&mut WindowManager) + 'static>>> =
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

/// Access `TheWindowManager` mutably.
///
/// On re-entry (RefCell already mutably borrowed) this does **not** create an overlapping
/// `&mut WindowManager` via `as_ptr()`. Unit (`R = ()`) callbacks are enqueued and run
/// when the outer borrow drains the queue. Non-unit returns are fail-closed (see
/// [`window_manager_reentry_fallback`]) without running `f`.
pub fn with_window_manager<R: 'static>(f: impl FnOnce(&mut WindowManager) -> R) -> R {
    THE_WINDOW_MANAGER.with(|manager| match manager.try_borrow_mut() {
        Ok(mut borrow) => {
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

fn window_manager_reentry<R: 'static>(f: impl FnOnce(&mut WindowManager) -> R) -> R {
    use std::any::TypeId;
    use std::mem;

    if TypeId::of::<R>() == TypeId::of::<()>() {
        // Enqueue the unit op. The stack owner drains this queue before returning
        // from the outer `with_window_manager` (and after input callbacks).
        //
        // SAFETY: `f` is treated as `'static` for TLS storage. Callers that re-enter
        // with `R = ()` must capture only data that outlives the outer
        // `with_window_manager` call (owned values / `Rc` / `'static`). `WindowLayout`
        // helpers clone `Rc`s for this reason. The queue is drained before that outer
        // call returns to *its* caller.
        let op: Box<dyn FnOnce(&mut WindowManager) + 'static> = unsafe {
            mem::transmute::<
                Box<dyn FnOnce(&mut WindowManager) + '_>,
                Box<dyn FnOnce(&mut WindowManager) + 'static>,
            >(Box::new(move |manager| {
                let _: R = f(manager);
            }))
        };
        WINDOW_MANAGER_OP_QUEUE.with(|queue| queue.borrow_mut().push(op));
        // SAFETY: `R` is `()` (checked via TypeId).
        return unsafe { mem::transmute_copy(&()) };
    }

    // Non-unit re-entry cannot be queued. Drop `f` and return a fail-closed default.
    drop(f);
    window_manager_reentry_fallback::<R>()
}

/// Fail-closed values for re-entrant `with_window_manager` when `R != ()`.
///
/// Documented defaults: `false`, `0`, `None`, `WindowInputReturnCode::NotUsed`,
/// `Err(WindowError::GeneralFailure)`. Unknown `R` panics so we never invent a
/// bit-pattern for an arbitrary type.
fn window_manager_reentry_fallback<R: 'static>() -> R {
    use std::any::TypeId;
    use std::mem;

    fn ret_if<R: 'static, T: 'static>(value: T) -> Option<R> {
        if TypeId::of::<R>() == TypeId::of::<T>() {
            // SAFETY: TypeId matched, so R == T.
            Some(unsafe { mem::transmute_copy::<T, R>(&value) })
        } else {
            None
        }
    }

    if let Some(v) = ret_if::<R, bool>(false) {
        return v;
    }
    if let Some(v) = ret_if::<R, i32>(0) {
        return v;
    }
    if let Some(v) = ret_if::<R, u32>(0) {
        return v;
    }
    if let Some(v) = ret_if::<R, usize>(0) {
        return v;
    }
    if let Some(v) = ret_if::<R, (i32, i32)>((0, 0)) {
        return v;
    }
    if let Some(v) = ret_if::<R, (u32, u32)>((0, 0)) {
        return v;
    }
    if let Some(v) = ret_if::<R, WindowInputReturnCode>(WindowInputReturnCode::NotUsed) {
        return v;
    }
    if let Some(v) = ret_if::<R, Option<Rc<RefCell<GameWindow>>>>(None) {
        return v;
    }
    if let Some(v) = ret_if::<R, Option<(i32, i32)>>(None) {
        return v;
    }
    if let Some(v) = ret_if::<R, Option<bool>>(None) {
        return v;
    }
    if let Some(v) = ret_if::<R, WindowResult<()>>(Err(WindowError::GeneralFailure)) {
        return v;
    }
    if let Some(v) = ret_if::<R, WindowResult<Rc<RefCell<GameWindow>>>>(Err(
        WindowError::GeneralFailure,
    )) {
        return v;
    }
    if let Some(v) = ret_if::<R, WindowResult<(Rc<RefCell<WindowLayout>>, WindowLayoutInfo)>>(Err(
        WindowError::GeneralFailure,
    )) {
        return v;
    }

    panic!(
        "re-entrant with_window_manager cannot fail-closed a value of type {}; \
         use a unit callback so the op can be queued",
        std::any::type_name::<R>()
    );
}

/// OS mouse → `TheWindowManager` hit-test + gadget input.
///
/// C++ `WindowXlat` turns `RAW_MOUSE_*` into `winSendInputMsg`. Shell active
/// consumes the click (LookAt/world command must not also fire).
pub fn dispatch_os_mouse_to_window_manager(
    msg: WindowMessage,
    x: i32,
    y: i32,
) -> WindowInputReturnCode {
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

pub(crate) fn apply_draw_callback_override(window_name: &str, draw: fn(&GameWindow, &WindowInstanceData)) {
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
