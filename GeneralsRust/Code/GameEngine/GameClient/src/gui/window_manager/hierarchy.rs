//! Window lookup, z-order, hide/enable, and enabled/hidden ancestry.
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
    /// Get window by ID
    pub fn get_window_by_id(&self, id: WindowId) -> Option<Rc<RefCell<GameWindow>>> {
        self.win_get_window_from_id(None, id)
    }

    /// Get the window list (root windows)
    pub fn get_window_list(&self) -> &[Rc<RefCell<GameWindow>>] {
        &self.root_windows
    }

    /// Get the total number of windows managed by this WindowManager.
    /// C++ parity: mirrors `TheWindowManager->winGetWindowList() != NULL` check.
    pub fn window_count(&self) -> usize {
        self.window_count
    }

    pub fn root_window_count(&self) -> usize {
        self.root_windows.len()
    }

    pub fn debug_collect_window_texts_by_prefix(
        &self,
        prefix: &str,
    ) -> Vec<(String, String, String, bool, Option<String>)> {
        fn collect(
            out: &mut Vec<(String, String, String, bool, Option<String>)>,
            prefix: &str,
            window: &Rc<RefCell<GameWindow>>,
        ) {
            let guard = window.borrow();
            if guard.get_name().starts_with(prefix) {
                let parent_name = guard
                    .get_parent()
                    .map(|parent| parent.borrow().get_name().to_string());
                out.push((
                    guard.get_name().to_string(),
                    guard.get_text().to_string(),
                    guard.get_text_label().to_string(),
                    guard.is_hidden(),
                    parent_name,
                ));
            }
            let children = guard.children().to_vec();
            drop(guard);
            for child in &children {
                collect(out, prefix, child);
            }
        }

        let mut out = Vec::new();
        for root in &self.root_windows {
            collect(&mut out, prefix, root);
        }
        out
    }

    pub fn debug_collect_window_draws_by_prefix(
        &self,
        prefix: &str,
    ) -> Vec<(
        String,
        bool,
        (i32, i32),
        (i32, i32),
        Option<String>,
        Option<String>,
    )> {
        fn collect(
            out: &mut Vec<(
                String,
                bool,
                (i32, i32),
                (i32, i32),
                Option<String>,
                Option<String>,
            )>,
            prefix: &str,
            window: &Rc<RefCell<GameWindow>>,
        ) {
            let guard = window.borrow();
            if guard.get_name().starts_with(prefix) {
                let parent_name = guard
                    .get_parent()
                    .map(|parent| parent.borrow().get_name().to_string());
                let image = guard
                    .get_enabled_draw_data(0)
                    .and_then(|entry| entry.image)
                    .map(|image| image.name);
                out.push((
                    guard.get_name().to_string(),
                    guard.is_hidden(),
                    guard.get_screen_position(),
                    guard.get_size(),
                    parent_name,
                    image,
                ));
            }
            let children = guard.children().to_vec();
            drop(guard);
            for child in &children {
                collect(out, prefix, child);
            }
        }

        let mut out = Vec::new();
        for root in &self.root_windows {
            collect(&mut out, prefix, root);
        }
        out
    }

    pub fn find_window_by_name(&self, name: &str) -> Option<Rc<RefCell<GameWindow>>> {
        fn find_recursive(
            name: &str,
            window: &Rc<RefCell<GameWindow>>,
        ) -> Option<Rc<RefCell<GameWindow>>> {
            let guard = window.borrow();
            if guard.get_name().eq_ignore_ascii_case(name) {
                return Some(window.clone());
            }
            let children = guard.children().to_vec();
            drop(guard);
            for child in &children {
                if let Some(found) = find_recursive(name, child) {
                    return Some(found);
                }
            }
            None
        }

        for root in &self.root_windows {
            if let Some(found) = find_recursive(name, root) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_window_from_id(
        &self,
        base_window: &Rc<RefCell<GameWindow>>,
        id: WindowId,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        self.win_get_window_from_id(Some(base_window), id)
    }

    pub fn win_get_window_from_id(
        &self,
        base_window: Option<&Rc<RefCell<GameWindow>>>,
        id: WindowId,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        fn find_in_window_and_descendants(
            window: &Rc<RefCell<GameWindow>>,
            id: WindowId,
        ) -> Option<Rc<RefCell<GameWindow>>> {
            let Ok(guard) = window.try_borrow() else {
                return None;
            };
            if guard.get_id() == id {
                return Some(window.clone());
            }
            let children = guard.children().to_vec();
            drop(guard);
            find_in_chain(&children, 0, id)
        }

        fn find_in_chain(
            windows: &[Rc<RefCell<GameWindow>>],
            start: usize,
            id: WindowId,
        ) -> Option<Rc<RefCell<GameWindow>>> {
            for window in windows.iter().skip(start) {
                if let Some(found) = find_in_window_and_descendants(window, id) {
                    return Some(found);
                }
            }
            None
        }

        if let Some(base_window) = base_window {
            if let Some(parent) = base_window.borrow().get_parent() {
                let siblings = parent.borrow().children().to_vec();
                if let Some(index) = siblings
                    .iter()
                    .position(|sibling| Rc::ptr_eq(sibling, base_window))
                {
                    return find_in_chain(&siblings, index, id);
                }
            } else if let Some(index) = self
                .root_windows
                .iter()
                .position(|root| Rc::ptr_eq(root, base_window))
            {
                return find_in_chain(&self.root_windows, index, id);
            }

            find_in_window_and_descendants(base_window, id)
        } else {
            find_in_chain(&self.root_windows, 0, id)
        }
    }

    pub fn bring_layout_forward(&mut self, layout: &WindowLayout) {
        for window in layout.windows.iter().rev() {
            self.bring_window_forward_internal(window, false);
        }
    }

    pub fn bring_window_forward(&mut self, window: &Rc<RefCell<GameWindow>>) {
        self.bring_window_forward_internal(window, true);
    }

    pub fn activate_window(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        self.bring_window_forward(window);
        window.borrow_mut().activate()
    }

    pub(crate) fn bring_window_forward_internal(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        update_layout: bool,
    ) {
        let mut moved = false;
        let parent = window.borrow().get_parent();
        if let Some(parent) = parent {
            let mut parent = parent.borrow_mut();
            let children = parent.children_mut();
            if let Some(index) = children.iter().position(|child| Rc::ptr_eq(child, window)) {
                let child = children.remove(index);
                children.insert(0, child);
                parent.sync_child_sibling_links();
                moved = true;
            }
        } else if let Some(index) = self
            .root_windows
            .iter()
            .position(|root| Rc::ptr_eq(root, window))
        {
            let root = self.root_windows.remove(index);
            self.add_root_window(root);
            moved = true;
        }

        if moved && update_layout {
            let layout = window.borrow().get_layout();
            if let Some(layout) = layout {
                layout.borrow_mut().bring_window_forward(window);
            }
        }
    }

    /// Hide windows in ID range
    pub fn hide_windows_in_range(
        &mut self,
        base_window: &Rc<RefCell<GameWindow>>,
        first: WindowId,
        last: WindowId,
        hide: bool,
    ) {
        for id in first..=last {
            if let Some(window) = self.find_window_from_id(base_window, id) {
                let _ = self.hide_window(&window, hide);
            }
        }
    }

    /// Hide or show a window with the manager side effects C++ applies from `winHide`.
    pub fn hide_window(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        hide: bool,
    ) -> WindowResult<()> {
        window
            .borrow_mut()
            .hide_without_manager_side_effects(hide)?;
        if hide {
            self.window_hiding(window);
        }
        Ok(())
    }

    pub(crate) fn window_hiding_from_direct_hide(
        &mut self,
        window_ptr: *const GameWindow,
        children: Vec<Rc<RefCell<GameWindow>>>,
    ) {
        // Prefer the live `Rc` (root / id map / provided children) so modal/focus
        // cleanup uses `Rc::ptr_eq` after a queued re-entry, not just a raw
        // pointer captured while `RefMut<GameWindow>` was held.
        let resolved = self
            .root_windows
            .iter()
            .cloned()
            .chain(self.window_by_id.values().filter_map(Weak::upgrade))
            .chain(children.iter().cloned())
            .find(|window| std::ptr::addr_eq(window.as_ptr(), window_ptr.cast_mut()));
        if let Some(window) = resolved {
            self.window_hiding(&window);
            return;
        }

        // Pointer may not match `RefCell::as_ptr()` if the hide was queued after
        // `RefMut` ended. If the provided children belong to the modal window,
        // that window is the one being hidden.
        if self.modal_stack.as_ref().is_some_and(|modal| {
            let modal_children = modal.window.borrow().children().to_vec();
            children
                .iter()
                .any(|child| modal_children.iter().any(|mc| Rc::ptr_eq(mc, child)))
        }) {
            if let Some(modal) = self.modal_stack.take() {
                self.modal_stack = modal.next;
            }
        }

        if self
            .keyboard_focus
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|focus| std::ptr::addr_eq(focus.as_ptr(), window_ptr.cast_mut()))
        {
            self.keyboard_focus = None;
        }

        if self
            .modal_stack
            .as_ref()
            .is_some_and(|modal| std::ptr::addr_eq(modal.window.as_ptr(), window_ptr.cast_mut()))
        {
            if let Some(modal) = self.modal_stack.take() {
                self.modal_stack = modal.next;
            }
        }

        if self
            .mouse_capture
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|capture| std::ptr::addr_eq(capture.as_ptr(), window_ptr.cast_mut()))
        {
            self.mouse_capture = None;
            self.capture_flags &= !CaptureFlags::MOUSE;
        }

        for child in children {
            self.window_hiding(&child);
        }
    }

    pub(crate) fn window_hiding(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if self
            .keyboard_focus
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|focus| Rc::ptr_eq(&focus, window))
        {
            self.keyboard_focus = None;
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
            .mouse_capture
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|capture| Rc::ptr_eq(&capture, window))
        {
            self.mouse_capture = None;
            self.capture_flags &= !CaptureFlags::MOUSE;
        }

        let children = match window.try_borrow() {
            Ok(win) => win.children().to_vec(),
            Err(_) => {
                // Nested RefCell during MainMenu hide / Start callback.
                // Next outer with_window_manager entry, not this drain.
                let window = window.clone();
                queue_window_manager_op_deferred(move |manager| {
                    manager.window_hiding(&window);
                });
                return;
            }
        };
        for child in children {
            self.window_hiding(&child);
        }
    }

    /// Enable windows in ID range
    pub fn enable_windows_in_range(
        &mut self,
        base_window: &Rc<RefCell<GameWindow>>,
        first: WindowId,
        last: WindowId,
        enable: bool,
    ) {
        for id in first..=last {
            if let Some(window) = self.find_window_from_id(base_window, id) {
                let _ = window.borrow_mut().enable(enable);
            }
        }
    }

    /// Check if window and all parents are enabled
    pub fn is_window_enabled(&self, window: &Rc<RefCell<GameWindow>>) -> bool {
        let mut current = Some(window.clone());
        while let Some(win) = current {
            let win_borrow = win.borrow();
            if !win_borrow.is_enabled() {
                return false;
            }
            current = win_borrow.get_parent();
        }
        true
    }

    /// Check if window or any parent is hidden
    pub fn is_window_hidden(&self, window: &Rc<RefCell<GameWindow>>) -> bool {
        let mut current = Some(window.clone());
        while let Some(win) = current {
            let win_borrow = win.borrow();
            if win_borrow.is_hidden() {
                return true;
            }
            current = win_borrow.get_parent();
        }
        false
    }
}
