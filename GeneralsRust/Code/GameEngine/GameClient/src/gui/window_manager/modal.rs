//! Modal window stack.
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
    /// Set modal window
    pub fn set_modal(&mut self, window: Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if window.borrow().get_parent().is_some() {
            return Err(WindowError::InvalidParameter);
        }

        let modal_window = Box::new(ModalWindow::new(window));

        // Push onto modal stack
        if let Some(old_head) = self.modal_stack.take() {
            let mut new_modal = modal_window;
            new_modal.next = Some(old_head);
            self.modal_stack = Some(new_modal);
        } else {
            self.modal_stack = Some(modal_window);
        }

        Ok(())
    }

    /// Remove modal window
    pub fn unset_modal(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if let Some(modal_head) = &self.modal_stack {
            if Rc::ptr_eq(&modal_head.window, window) {
                self.modal_stack = modal_head.next.clone();
                return Ok(());
            }
        }
        Err(WindowError::GeneralFailure)
    }
}
