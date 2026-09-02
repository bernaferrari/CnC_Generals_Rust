//! Window create, destroy, parent/owner, and destroy-queue processing.
#![allow(unused_imports)]

use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    RadioButtonGroup, StaticText, TabControl, TextEntry, VerticalSlider,
};
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
    /// Create a new window
    pub fn create_window(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        let window_id = generate_window_id();
        self.create_window_with_id(parent, x, y, width, height, window_id)
    }

    /// Create a new window with an explicit ID.
    pub fn create_window_with_id(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        window_id: WindowId,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        self.create_window_with_id_internal(parent, x, y, width, height, window_id, true)
    }

    pub(crate) fn create_window_with_id_internal(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        window_id: WindowId,
        send_create: bool,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        if self.window_count >= MAX_WINDOWS {
            return Err(WindowError::OutOfWindows);
        }

        let window = Rc::new(RefCell::new(GameWindow::new()));

        // Set up window properties
        {
            let mut window_mut = window.borrow_mut();
            window_mut.set_id(window_id);
            window_mut.set_position(x, y)?;
            window_mut.set_size(width, height)?;
            window_mut.enable(true)?;
        }

        // Add to parent or root list
        if let Some(parent_rc) = parent {
            window.borrow_mut().set_parent(Some(parent_rc));
            window.borrow_mut().set_owner(Some(parent_rc));
            parent_rc.borrow_mut().add_child(window.clone());
        } else {
            window.borrow_mut().set_owner_self(&window);
            self.add_root_window(window.clone());
        }

        // Register in lookup table
        self.window_by_id.insert(window_id, Rc::downgrade(&window));
        self.window_count += 1;

        // Send create message
        if send_create {
            let _msg_result = window
                .borrow_mut()
                .send_system_message(WindowMessage::Create, 0, 0);
        }

        Ok(window)
    }

    /// Destroy a window
    pub fn destroy_window(&mut self, window: Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        // Add to destroy queue for safe deletion
        self.destroy_queue.push_back(window);
        Ok(())
    }

    /// Reparent a managed window using the same unlink/link ordering as C++ winSetParent.
    pub fn set_window_parent(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        parent: Option<&Rc<RefCell<GameWindow>>>,
    ) -> WindowResult<()> {
        let old_parent = window.borrow().get_parent();
        if let Some(old_parent) = old_parent {
            old_parent.borrow_mut().remove_child(window);
        } else {
            self.root_windows.retain(|root| !Rc::ptr_eq(root, window));
            self.sync_root_sibling_links();
        }

        if let Some(parent) = parent {
            window.borrow_mut().set_parent(Some(parent));
            parent.borrow_mut().add_child(window.clone());
        } else {
            window.borrow_mut().set_parent(None);
            self.add_root_window(window.clone());
        }

        Ok(())
    }

    /// Set a window's owner using C++ winSetOwner(NULL) semantics.
    pub fn set_window_owner(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        owner: Option<&Rc<RefCell<GameWindow>>>,
    ) -> WindowResult<()> {
        if let Some(owner) = owner {
            window.borrow_mut().set_owner(Some(owner));
        } else {
            window.borrow_mut().set_owner_self(window);
        }
        Ok(())
    }

    /// Destroy all windows
    pub fn destroy_all_windows(&mut self) {
        log::warn!(
            "destroy_all_windows: roots={} layouts={}",
            self.root_windows.len(),
            self.layouts.len()
        );
        // Add all root windows to destroy queue
        for window in self.root_windows.drain(..) {
            self.destroy_queue.push_back(window);
        }

        // Process destroy queue
        self.process_destroy_queue();
    }

    /// Process any queued window destruction immediately.
    pub fn flush_destroy_queue(&mut self) {
        self.process_destroy_queue();
    }

    /// Add window to root window list
    pub(crate) fn add_root_window(&mut self, window: Rc<RefCell<GameWindow>>) {
        if let Some(last_modal_index) = self.last_modal_root_index(&window) {
            self.root_windows.insert(last_modal_index + 1, window);
        } else {
            self.root_windows.insert(0, window);
        }
        self.sync_root_sibling_links();
    }

    pub(crate) fn sync_root_sibling_links(&self) {
        GameWindow::sync_sibling_links(&self.root_windows);
    }

    pub(crate) fn last_modal_root_index(&self, window: &Rc<RefCell<GameWindow>>) -> Option<usize> {
        let mut last_modal_index: Option<usize> = None;
        let mut modal = self.modal_stack.as_deref();
        while let Some(entry) = modal {
            if !Rc::ptr_eq(&entry.window, window) {
                if let Some(index) = self
                    .root_windows
                    .iter()
                    .position(|root| Rc::ptr_eq(root, &entry.window))
                {
                    last_modal_index = Some(last_modal_index.map_or(index, |last| last.max(index)));
                }
            }
            modal = entry.next.as_deref();
        }
        last_modal_index
    }

    /// Process the destroy queue
    pub(crate) fn process_destroy_queue(&mut self) {
        let mut destroy_notifications = Vec::new();
        while let Some(window) = self.destroy_queue.pop_front() {
            self.destroy_window_immediate(window, &mut destroy_notifications);
        }

        for window in destroy_notifications {
            window
                .borrow_mut()
                .send_system_message(WindowMessage::Destroy, 0, 0);
        }
    }

    /// Immediately destroy a window
    pub(crate) fn destroy_window_immediate(
        &mut self,
        window: Rc<RefCell<GameWindow>>,
        destroy_notifications: &mut Vec<Rc<RefCell<GameWindow>>>,
    ) {
        debug_assert!(
            window.borrow().get_edit_data().is_none(),
            "winDestroy(): edit data should NOT be present!"
        );

        if window
            .borrow()
            .get_status()
            .contains(WindowStatus::DESTROYED)
        {
            return;
        }

        let window_id = window.borrow().get_id();
        let status = window.borrow().get_status() | WindowStatus::DESTROYED;
        window.borrow_mut().set_status_exact(status);

        // Remove from various manager references
        self.clear_references_to_destroyed_window(&window);

        let children = window.borrow().children().to_vec();
        for child in children {
            self.destroy_window_immediate(child, destroy_notifications);
        }

        // Remove from parent's children or root list
        let parent = window.borrow().get_parent();
        if let Some(parent) = parent {
            parent.borrow_mut().remove_child(&window);
        } else {
            self.root_windows.retain(|w| !Rc::ptr_eq(w, &window));
            self.sync_root_sibling_links();
        }

        // Remove from lookup table
        self.window_by_id.remove(&window_id);

        // Remove from owning layout
        let layout = window.borrow().get_layout();
        if let Some(layout) = layout {
            layout.borrow_mut().remove_window(&window);
            window.borrow_mut().set_layout(None);
        }

        // C++ winDestroy adds each removed window to the head of m_destroyList;
        // processDestroyList then sends GWM_DESTROY in that head-first order.
        destroy_notifications.insert(0, window);

        self.window_count = self.window_count.saturating_sub(1);
    }

    pub(crate) fn clear_references_to_destroyed_window(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
    ) {
        if self
            .keyboard_focus
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|focus| Rc::ptr_eq(&focus, window))
        {
            self.keyboard_focus = None;
        }

        if self
            .mouse_capture
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|capture| Rc::ptr_eq(&capture, window))
        {
            self.mouse_capture = None;
            self.capture_flags &= !CaptureFlags::MOUSE;
        }

        if self
            .modal_stack
            .as_ref()
            .is_some_and(|modal| Rc::ptr_eq(&modal.window, window))
        {
            if let Some(modal) = self.modal_stack.take() {
                self.modal_stack = modal.next;
            }
        }

        if self
            .current_mouse_region
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|region| Rc::ptr_eq(&region, window))
        {
            self.current_mouse_region = None;
        }

        if self
            .grab_window
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|grab| Rc::ptr_eq(&grab, window))
        {
            self.grab_window = None;
        }
    }
}
