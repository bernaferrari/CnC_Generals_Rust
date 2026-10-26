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

/// Read the canonical callback window directly; all other entries retain their
/// ordinary arena borrow. No busy entry is interpreted as an absent subtree.
fn with_lookup_window<R>(
    window: &Rc<RefCell<GameWindow>>,
    selected: Option<&GameWindow>,
    read: impl FnOnce(&GameWindow) -> R,
) -> R {
    if let Some(selected) =
        selected.filter(|selected| std::ptr::eq(window.as_ptr(), std::ptr::from_ref(*selected)))
    {
        read(selected)
    } else {
        read(&window.borrow())
    }
}

fn lookup_window_chain(
    windows: &[Rc<RefCell<GameWindow>>],
    start: usize,
    matches: &impl Fn(&GameWindow) -> bool,
    selected: Option<&GameWindow>,
) -> Option<Rc<RefCell<GameWindow>>> {
    for window in windows.iter().skip(start) {
        let (is_match, children) = with_lookup_window(window, selected, |value| {
            (matches(value), value.children().to_vec())
        });
        if is_match {
            return Some(window.clone());
        }
        if let Some(found) = lookup_window_chain(&children, 0, matches, selected) {
            return Some(found);
        }
    }
    None
}

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
        self.find_window_by_name_with_selected(name, None)
    }

    pub fn find_window_by_name_with_selected(
        &self,
        name: &str,
        selected: Option<&GameWindow>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        lookup_window_chain(
            &self.root_windows,
            0,
            &|window| window.get_name().eq_ignore_ascii_case(name),
            selected,
        )
    }

    pub fn get_window_by_id_with_selected(
        &self,
        id: WindowId,
        selected: Option<&GameWindow>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        self.win_get_window_from_id_with_selected(None, id, selected)
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
        self.win_get_window_from_id_with_selected(base_window, id, None)
    }

    /// C++ starts at the supplied window and continues through its following
    /// siblings, descending through each child list before the next sibling.
    pub fn win_get_window_from_id_with_selected(
        &self,
        base_window: Option<&Rc<RefCell<GameWindow>>>,
        id: WindowId,
        selected: Option<&GameWindow>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let matches = |window: &GameWindow| window.get_id() == id;
        if let Some(base_window) = base_window {
            let parent = with_lookup_window(base_window, selected, |window| window.get_parent());
            if let Some(parent) = parent {
                let siblings =
                    with_lookup_window(&parent, selected, |window| window.children().to_vec());
                if let Some(index) = siblings
                    .iter()
                    .position(|sibling| Rc::ptr_eq(sibling, base_window))
                {
                    return lookup_window_chain(&siblings, index, &matches, selected);
                }
            } else if let Some(index) = self
                .root_windows
                .iter()
                .position(|root| Rc::ptr_eq(root, base_window))
            {
                return lookup_window_chain(&self.root_windows, index, &matches, selected);
            }
            lookup_window_chain(std::slice::from_ref(base_window), 0, &matches, selected)
        } else {
            lookup_window_chain(&self.root_windows, 0, &matches, selected)
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
                // C++ removeWindow + addWindow leaves an existing layout head
                // in the same position with the same links. Keep the manager
                // promotion above immediate, but do not take an unnecessary
                // exclusive layout loan: run_init callbacks may hold its
                // shared observation while activating that very first root.
                let already_first = layout
                    .borrow()
                    .get_first_window()
                    .as_ref()
                    .is_some_and(|first| Rc::ptr_eq(first, window));
                if !already_first {
                    layout.borrow_mut().bring_window_forward(window);
                }
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

    /// C++ winHide: update canonical visibility before focus/modal/capture cleanup.
    pub fn hide_window(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        hide: bool,
    ) -> WindowResult<()> {
        let (_, _, mut access) = self.transition_parts(None);
        access.hide_window(window, hide);
        Ok(())
    }
    /// Visibility operations may traverse the currently dispatched window's
    /// descendants; carry its canonical loan through cleanup as well as lookup.
    pub fn hide_window_with_selected(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        hide: bool,
        selected: Option<&mut GameWindow>,
    ) -> WindowResult<()> {
        let (_, _, mut access) = self.transition_parts(selected);
        access.hide_window(window, hide);
        Ok(())
    }
    pub(crate) fn window_hiding_from_direct_hide(
        &mut self,
        window_ptr: *const GameWindow,
        children: Vec<Rc<RefCell<GameWindow>>>,
    ) {
        let (_, _, mut access) = self.transition_parts(None);
        access.hiding(window_ptr, children);
    }
    pub(crate) fn window_hiding(&mut self, window: &Rc<RefCell<GameWindow>>) {
        let children = window.borrow().children().to_vec();
        self.window_hiding_from_direct_hide(window.as_ptr(), children);
    }

    /// Visit the source sibling order while preserving the already-borrowed
    /// canonical callback window, including a selected child of this parent.
    pub fn enable_window_with_selected(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        enable: bool,
        mut selected: Option<&mut GameWindow>,
    ) -> WindowResult<()> {
        let children = {
            let mut window =
                super::transition_access::borrow_callback_window(window, selected.as_deref_mut());
            window.set_enabled_status(enable);
            window.children().to_vec()
        };
        for child in children {
            self.enable_window_with_selected(&child, enable, selected.as_deref_mut())?;
        }
        Ok(())
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

#[cfg(test)]
#[path = "layout_head_activation_tests.rs"]
mod layout_head_activation_tests;
