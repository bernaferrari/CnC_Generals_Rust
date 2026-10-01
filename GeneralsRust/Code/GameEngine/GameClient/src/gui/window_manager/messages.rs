//! System and input message forwarding.
#![allow(unused_imports)]

use crate::gui::game_window::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use super::*;

impl WindowManager {
    // Re-entrancy contract for message forwarding (winSend*Msg ports):
    // the `borrow_mut()` is a temporary that lives only for the send
    // expression; the invoked input/system callback receives `&GameWindow`,
    // so it cannot re-borrow this RefCell through its arguments. Callbacks
    // that must mutate *this* window mid-dispatch go through
    // queue_window_manager_op / try_borrow (see layout.rs bring_forward and
    // window_impl_input.rs grab-window handling) rather than a nested
    // borrow_mut, which would panic while this guard is live. Keeping the
    // borrow scoped to the single send statement preserves C++ ordering.

    /// Send system message to window
    pub fn send_system_message(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        window.borrow_mut().send_system_message(msg, data1, data2)
    }

    /// Send input message to window
    pub fn send_input_message(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        window.borrow_mut().send_input_message(msg, data1, data2)
    }
}
