//! Keyboard focus and tab-list navigation.
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
    /// Set keyboard focus to a window
    pub fn set_focus(&mut self, window: Option<&Rc<RefCell<GameWindow>>>) -> WindowResult<()> {
        if let Some(new_focus) = window {
            let no_focus = match new_focus.try_borrow() {
                Ok(win) => win.get_status().contains(WindowStatus::NO_FOCUS),
                Err(_) => false,
            };
            if no_focus {
                return Ok(());
            }
        }

        // Clear old focus
        if let Some(old_focus_weak) = &self.keyboard_focus {
            if let Some(old_focus) = old_focus_weak.upgrade() {
                let changing_focus = window
                    .map(|new_focus| !Rc::ptr_eq(&old_focus, new_focus))
                    .unwrap_or(true);
                if changing_focus {
                    let token = push_payload(WindowMsgPayload::Bool(false));
                    old_focus
                        .borrow_mut()
                        .send_system_message(WindowMessage::InputFocus, 0, token);
                    let _ = pop_payload(token);
                }
            }
        }

        // Set new focus
        if let Some(new_focus) = window {
            self.keyboard_focus = Some(Rc::downgrade(new_focus));

            let mut wants_focus = false;
            let mut focus_probe = Some(new_focus.clone());
            while let Some(window) = focus_probe {
                let token = push_payload(WindowMsgPayload::Bool(false));
                window
                    .borrow_mut()
                    .send_system_message(WindowMessage::InputFocus, 1, token);
                wants_focus = matches!(pop_payload(token), Some(WindowMsgPayload::Bool(true)));
                if wants_focus {
                    break;
                }
                focus_probe = window.borrow().get_parent();
            }

            if !wants_focus {
                self.keyboard_focus = None;
            }
        } else {
            self.keyboard_focus = None;
        }

        Ok(())
    }

    pub fn request_focus(&mut self, id: WindowId) {
        self.pending_focus = Some(id);
    }

    /// Get window that has keyboard focus
    pub fn get_focus(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.keyboard_focus.as_ref()?.upgrade()
    }

    /// Navigate to next/previous tab
    pub fn navigate_tab(&mut self, direction: TabDirection) {
        if self.tab_list.is_empty() || self.modal_stack.is_some() {
            return;
        }

        let current_focus = self.get_focus();
        let mut next_window = None;

        // Clean up dead references
        self.tab_list.retain(|w| w.upgrade().is_some());
        if self.tab_list.is_empty() {
            return;
        }

        if let Some(current) = current_focus {
            // Find current window in tab list
            let current_ptr = current.as_ptr();
            let current_index = self
                .tab_list
                .iter()
                .position(|w| w.upgrade().map(|rc| rc.as_ptr()) == Some(current_ptr));

            if let Some(index) = current_index {
                let next_index = match direction {
                    TabDirection::Next => (index + 1) % self.tab_list.len(),
                    TabDirection::Previous => {
                        if index == 0 {
                            self.tab_list.len() - 1
                        } else {
                            index - 1
                        }
                    }
                };

                next_window = self.tab_list[next_index].upgrade();
            }
        }

        // If no current focus or not in tab list, mirror C++ wrap fallback.
        if next_window.is_none() {
            next_window = match direction {
                TabDirection::Next => self.tab_list.first().and_then(Weak::upgrade),
                TabDirection::Previous => self.tab_list.last().and_then(Weak::upgrade),
            };
        }

        if let Some(window) = next_window {
            let _ = self.set_focus(Some(&window));
            self.set_lone_window(None);
        }
    }

    /// Register tab list
    pub fn register_tab_list(&mut self, windows: Vec<Rc<RefCell<GameWindow>>>) {
        self.tab_list = windows.into_iter().map(|w| Rc::downgrade(&w)).collect();
    }

    /// Clear tab list
    pub fn clear_tab_list(&mut self) {
        self.tab_list.clear();
    }
}
