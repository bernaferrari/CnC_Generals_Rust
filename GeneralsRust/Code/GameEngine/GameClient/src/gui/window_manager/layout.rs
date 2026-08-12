//! WindowLayout grouping and script-load info.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::gui::game_window::{Color, GameFont, GameWindow};

use super::{queue_window_manager_op, queue_window_manager_op_deferred};

/// Window layout for grouping related windows
pub struct WindowLayout {
    filename: String,
    pub(crate) windows: Vec<Rc<RefCell<GameWindow>>>,
    self_handle: Weak<RefCell<WindowLayout>>,
    hidden: Cell<bool>,
    pub(crate) default_text_color: Option<Color>,
    pub(crate) default_font: Option<GameFont>,
    // Layout callbacks would be function pointers in the original
    pub(crate) init_callback: Option<Box<dyn Fn(&WindowLayout, Option<&dyn std::any::Any>)>>,
    pub(crate) update_callback: Option<Box<dyn Fn(&WindowLayout, Option<&dyn std::any::Any>)>>,
    pub(crate) shutdown_callback:
        Option<Box<dyn Fn(&WindowLayout, Option<&mut dyn std::any::Any>)>>,
}

impl std::fmt::Debug for WindowLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowLayout")
            .field("filename", &self.filename)
            .field("window_count", &self.windows.len())
            .field("hidden", &self.hidden.get())
            .finish()
    }
}

impl WindowLayout {
    pub fn new(filename: String) -> Self {
        Self {
            filename,
            windows: Vec::new(),
            self_handle: Weak::new(),
            hidden: Cell::new(false),
            default_text_color: None,
            default_font: None,
            init_callback: None,
            update_callback: None,
            shutdown_callback: None,
        }
    }

    pub(crate) fn set_self_handle(&mut self, layout: &Rc<RefCell<WindowLayout>>) {
        self.self_handle = Rc::downgrade(layout);
    }

    /// Get the filename associated with this layout
    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    /// Check if layout is hidden
    pub fn is_hidden(&self) -> bool {
        self.hidden.get()
    }

    /// Hide or show all windows in this layout
    pub fn hide(&self, hide: bool) {
        for window_rc in &self.windows {
            // Clone the Rc so a re-entrant call can queue this op without
            // capturing a borrow into `self.windows`.
            let window = window_rc.clone();
            queue_window_manager_op(move |manager| {
                let _ = manager.hide_window(&window, hide);
            });
        }
        self.hidden.set(hide);
    }

    /// Add window to this layout
    pub fn add_window(&mut self, window: Rc<RefCell<GameWindow>>) {
        // Check if window is already in layout
        let window_ptr = window.as_ptr();
        if !self.windows.iter().any(|w| w.as_ptr() == window_ptr) {
            if let Some(layout) = self.self_handle.upgrade() {
                window.borrow_mut().set_layout(Some(&layout));
            }
            self.windows.insert(0, window);
            self.sync_window_layout_links();
        }
    }

    /// Access windows owned by this layout.
    pub fn windows(&self) -> &[Rc<RefCell<GameWindow>>] {
        &self.windows
    }

    /// Remove window from this layout
    pub fn remove_window(&mut self, window: &Rc<RefCell<GameWindow>>) {
        let window_ptr = window.as_ptr();
        if let Some(index) = self.windows.iter().position(|w| w.as_ptr() == window_ptr) {
            let removed = self.windows.remove(index);
            if let Some(layout) = self.self_handle.upgrade() {
                let owns_window = removed
                    .borrow()
                    .get_layout()
                    .as_ref()
                    .is_some_and(|window_layout| Rc::ptr_eq(window_layout, &layout));
                if owns_window {
                    removed.borrow_mut().set_layout(None);
                }
            }
            {
                let mut removed = removed.borrow_mut();
                removed.set_next_in_layout(None);
                removed.set_prev_in_layout(None);
            }
            self.sync_window_layout_links();
        }
    }

    /// Move an existing layout window to the front of the layout list.
    pub fn bring_window_forward(&mut self, window: &Rc<RefCell<GameWindow>>) {
        let window_ptr = window.as_ptr();
        if let Some(index) = self.windows.iter().position(|w| w.as_ptr() == window_ptr) {
            let window = self.windows.remove(index);
            self.windows.insert(0, window);
            self.sync_window_layout_links();
        }
    }

    fn sync_window_layout_links(&self) {
        for (index, window) in self.windows.iter().enumerate() {
            let prev = index.checked_sub(1).and_then(|prev| self.windows.get(prev));
            let next = self.windows.get(index + 1);
            let mut window = window.borrow_mut();
            window.set_prev_in_layout(prev);
            window.set_next_in_layout(next);
        }
    }

    /// Get first window in layout
    pub fn get_first_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.windows.first().cloned()
    }

    /// Bring all windows in this layout to front
    pub fn bring_forward(&mut self) {
        if let Some(layout) = self.self_handle.upgrade() {
            queue_window_manager_op(move |manager| {
                // Shell::push may still hold a mutable layout borrow when this
                // op drains; try_borrow avoids a RefCell panic that aborted
                // Menu entry on the windowed sit-through path.
                match layout.try_borrow() {
                    Ok(borrowed) => manager.bring_layout_forward(&borrowed),
                    Err(_) => {
                        let layout = layout.clone();
                        queue_window_manager_op_deferred(move |manager| {
                            if let Ok(borrowed) = layout.try_borrow() {
                                manager.bring_layout_forward(&borrowed);
                            }
                        });
                    }
                }
            });
        }
    }

    /// Run initialization callback
    pub fn run_init(&self, user_data: Option<&dyn std::any::Any>) {
        if let Some(ref callback) = self.init_callback {
            callback(self, user_data);
        }
    }

    /// Run update callback
    pub fn run_update(&self, user_data: Option<&dyn std::any::Any>) {
        if let Some(ref callback) = self.update_callback {
            callback(self, user_data);
        }
    }

    /// Run shutdown callback
    pub fn run_shutdown(&self, user_data: Option<&mut dyn std::any::Any>) {
        if let Some(ref callback) = self.shutdown_callback {
            callback(self, user_data);
        }
    }

    /// Destroy all windows in this layout
    /// Detach all windows from this layout without destroying them.
    pub fn take_windows(&mut self) -> Vec<Rc<RefCell<GameWindow>>> {
        std::mem::take(&mut self.windows)
    }

    pub fn destroy_windows(&mut self) {
        let windows = self.windows.clone();

        queue_window_manager_op(move |manager| {
            for window in windows {
                let _ = manager.destroy_window(window);
            }
            manager.flush_destroy_queue();
        });

        self.windows.clear();
    }
}

/// Layout information returned from script loading
#[derive(Debug, Default)]
pub struct WindowLayoutInfo {
    pub version: u32,
    pub init_callback_name: String,
    pub update_callback_name: String,
    pub shutdown_callback_name: String,
    pub windows: Vec<Rc<RefCell<GameWindow>>>,
}
