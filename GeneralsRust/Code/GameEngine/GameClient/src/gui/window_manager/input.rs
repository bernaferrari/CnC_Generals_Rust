//! Mouse/key dispatch, capture, grab, lone-window, and hit testing.
#![allow(unused_imports)]

use crate::gui::game_window::*;
use crate::input::with_mouse;
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
    /// Capture mouse input to a window
    pub fn capture_mouse(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if self.mouse_capture.is_some() {
            return Err(WindowError::MouseCaptured);
        }

        self.mouse_capture = Some(Rc::downgrade(window));
        self.capture_flags |= CaptureFlags::MOUSE;
        Ok(())
    }

    /// Release mouse capture
    pub fn release_capture(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if let Some(capture_weak) = &self.mouse_capture {
            if let Some(capture_window) = capture_weak.upgrade() {
                if Rc::ptr_eq(&capture_window, window) {
                    self.mouse_capture = None;
                    self.capture_flags &= !CaptureFlags::MOUSE;
                }
            }
        }
        Ok(())
    }

    /// Get window that has mouse capture
    pub fn get_capture(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.mouse_capture.as_ref()?.upgrade()
    }

    /// Set grab window (for drag operations)
    pub fn set_grab_window(&mut self, window: Option<&Rc<RefCell<GameWindow>>>) {
        self.grab_window = window.map(Rc::downgrade);
    }

    /// Get grab window
    pub fn get_grab_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.grab_window.as_ref()?.upgrade()
    }

    /// Set lone window (for exclusive operations like combo boxes)
    pub fn set_lone_window(&mut self, window: Option<&Rc<RefCell<GameWindow>>>) {
        const GGM_LEFT_DRAG: u32 = 16384;
        const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;
        if let Some(old) = self.lone_window.as_ref().and_then(|w| w.upgrade()) {
            let same = window.map(|w| Rc::ptr_eq(&old, w)).unwrap_or(false);
            if !same {
                let _ = old
                    .borrow_mut()
                    .send_system_message(WindowMessage::User(GGM_CLOSE), 0, 0);
            }
        }
        self.lone_window = window.map(Rc::downgrade);
    }

    /// Process mouse event
    pub fn process_mouse_event(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
    ) -> WindowInputReturnCode {
        self.process_mouse_event_with_delta(msg, x, y, data, None)
    }

    /// Process mouse event with optional mouse delta for legacy drag handling.
    pub fn process_mouse_event_with_delta(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
        mouse_delta: Option<(i32, i32)>,
    ) -> WindowInputReturnCode {
        const GGM_LEFT_DRAG: u32 = 16384;
        const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;
        let old_lone = self.lone_window.as_ref().and_then(|w| w.upgrade());
        self.update_cursor_tooltip_for_mouse_event(x, y);
        // Find window under cursor or use capture
        let capture_window = self.get_capture();
        let target_window = if let Some(capture) = capture_window.as_ref() {
            self.set_grab_window(None);
            Some(self.find_child_at_point_or_self(capture, x, y, false))
        } else {
            self.get_input_window_under_cursor(x, y)
        };

        if self.get_capture().is_none() {
            if let Some(grab_window) = self.get_grab_window() {
                match msg {
                    WindowMessage::LeftUp => {
                        // C++ winProcessMouseEvent: m_grabWindow->winPointInChild(x, y, FALSE, TRUE)
                        let _ = GameWindow::point_in_child_ex(&grab_window, x, y, false, true);
                        let should_send_release = {
                            let mut window = grab_window.borrow_mut();
                            window.clear_status(WindowStatus::ACTIVE);
                            window.point_in_window(x, y)
                                || window.get_status().contains(WindowStatus::DRAGABLE)
                        };

                        if should_send_release {
                            let _ = grab_window.borrow_mut().send_input_message(msg, data, 0);
                        }
                        self.set_grab_window(None);
                        return WindowInputReturnCode::Used;
                    }
                    WindowMessage::None | WindowMessage::LeftDrag => {
                        if let Some((dx, dy)) = mouse_delta {
                            self.move_grabbed_draggable_window(&grab_window, dx, dy);
                        }
                        let _ = grab_window.borrow_mut().send_input_message(msg, data, 0);
                        return WindowInputReturnCode::Used;
                    }
                    _ => {
                        return WindowInputReturnCode::Used;
                    }
                }
            }
        }

        if let Some(window) = target_window {
            let handled_window =
                self.send_mouse_message_up_chain(&window, msg, data, x, y, capture_window.as_ref());

            self.update_current_mouse_region(Some(&window), capture_window.as_ref(), x, y);

            self.close_lone_window_after_mouse_message(
                old_lone.as_ref(),
                handled_window.as_ref(),
                msg,
            );

            if msg == WindowMessage::LeftDown {
                if let Some(handled) = handled_window.as_ref() {
                    self.set_grab_window(Some(handled));
                }
            }

            if handled_window.is_some() {
                WindowInputReturnCode::Used
            } else {
                WindowInputReturnCode::NotUsed
            }
        } else {
            self.update_current_mouse_region(None, capture_window.as_ref(), x, y);
            if matches!(
                msg,
                WindowMessage::LeftUp | WindowMessage::MiddleUp | WindowMessage::RightUp
            ) {
                self.close_lone_window_after_mouse_message(old_lone.as_ref(), None, msg);
            }
            WindowInputReturnCode::NotUsed
        }
    }

    pub(crate) fn update_current_mouse_region(
        &mut self,
        new_window: Option<&Rc<RefCell<GameWindow>>>,
        capture_window: Option<&Rc<RefCell<GameWindow>>>,
        x: i32,
        y: i32,
    ) {
        let previous = self.current_mouse_region.as_ref().and_then(|w| w.upgrade());
        let same = match (&previous, new_window) {
            (Some(prev), Some(cur)) => Rc::ptr_eq(prev, cur),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }

        if let Some(prev) = previous {
            let should_send_leaving = capture_window
                .map(|capture| Self::is_strict_descendant(capture, &prev))
                .unwrap_or(true);
            if should_send_leaving {
                let (px, py) = prev.borrow().get_screen_position();
                let _ = prev.borrow_mut().set_cursor_position(x - px, y - py);
                let _ =
                    prev.borrow_mut()
                        .send_routed_input_message(WindowMessage::MouseLeaving, 0, 0);
            }
        }

        if let Some(new_window) = new_window {
            let (wx, wy) = new_window.borrow().get_screen_position();
            let _ = new_window.borrow_mut().set_cursor_position(x - wx, y - wy);
            let _ = new_window.borrow_mut().send_routed_input_message(
                WindowMessage::MouseEntering,
                0,
                0,
            );
            self.current_mouse_region = Some(Rc::downgrade(new_window));
        } else {
            self.current_mouse_region = None;
        }
    }

    pub(crate) fn close_lone_window_after_mouse_message(
        &mut self,
        old_lone: Option<&Rc<RefCell<GameWindow>>>,
        handled_window: Option<&Rc<RefCell<GameWindow>>>,
        msg: WindowMessage,
    ) {
        let Some(old_lone) = old_lone else {
            return;
        };

        let current_lone_is_unchanged = self
            .lone_window
            .as_ref()
            .and_then(|w| w.upgrade())
            .is_some_and(|current| Rc::ptr_eq(&current, old_lone));
        if !current_lone_is_unchanged {
            return;
        }

        let mouse_up = matches!(
            msg,
            WindowMessage::LeftUp | WindowMessage::MiddleUp | WindowMessage::RightUp
        );
        if !mouse_up && handled_window.is_none() {
            return;
        }

        if handled_window.is_some_and(|handled| Self::is_strict_descendant(old_lone, handled)) {
            return;
        }

        self.set_lone_window(None);
    }

    pub(crate) fn is_strict_descendant(
        parent: &Rc<RefCell<GameWindow>>,
        child: &Rc<RefCell<GameWindow>>,
    ) -> bool {
        let mut current = child.borrow().get_parent();
        while let Some(window) = current {
            if Rc::ptr_eq(&window, parent) {
                return true;
            }
            current = window.borrow().get_parent();
        }
        false
    }

    pub(crate) fn send_mouse_message_up_chain(
        &mut self,
        start: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data: WindowMsgData,
        x: i32,
        y: i32,
        stop_at: Option<&Rc<RefCell<GameWindow>>>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let mut current = Some(start.clone());
        while let Some(window) = current {
            let (wx, wy) = window.borrow().get_screen_position();
            let result = {
                let mut window_ref = window.borrow_mut();
                let _ = window_ref.set_cursor_position(x - wx, y - wy);
                if stop_at.is_some() {
                    window_ref.send_routed_input_message(msg, data, 0)
                } else {
                    window_ref.send_input_message(msg, data, 0)
                }
            };

            if result.is_handled() {
                return Some(window);
            }

            if stop_at.is_some_and(|stop| Rc::ptr_eq(stop, &window)) {
                break;
            }

            current = window.borrow().get_parent();
        }

        None
    }

    pub(crate) fn move_grabbed_draggable_window(
        &self,
        grab_window: &Rc<RefCell<GameWindow>>,
        mut dx: i32,
        mut dy: i32,
    ) {
        let (x, y, width, height, parent_size) = {
            let window = grab_window.borrow();
            if !window.get_status().contains(WindowStatus::DRAGABLE) {
                return;
            }

            let parent_size = window.get_parent().map(|parent| parent.borrow().get_size());
            let (x, y) = window.get_position();
            let (width, height) = window.get_size();
            (x, y, width, height, parent_size)
        };

        if let Some((parent_width, parent_height)) = parent_size {
            if x + dx < 0 {
                dx = -x;
            } else if x + width + dx > parent_width {
                dx = parent_width - (x + width);
            }

            if y + dy < 0 {
                dy = -y;
            } else if y + height + dy > parent_height {
                dy = parent_height - (y + height);
            }
        }

        let (screen_width, screen_height) = self.screen_size;
        let mut new_x = x + dx;
        let mut new_y = y + dy;
        if new_x < 0 {
            new_x = 0;
        }
        if new_y < 0 {
            new_y = 0;
        }

        let mut high_x = new_x + width;
        let mut high_y = new_y + height;
        if high_x > screen_width {
            high_x = screen_width;
        }
        if high_y > screen_height {
            high_y = screen_height;
        }

        new_x = high_x - width;
        new_y = high_y - height;
        let _ = grab_window.borrow_mut().set_position(new_x, new_y);
    }

    pub(crate) fn update_cursor_tooltip_for_mouse_event(&self, x: i32, y: i32) {
        with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));

        if self.get_capture().is_some() || self.get_grab_window().is_some() {
            return;
        }

        let Some(window_rc) = self.find_tooltip_window_at_point(x, y) else {
            return;
        };
        let packed = ((y as u32) << 16) | ((x as u32) & 0xffff);
        let window = window_rc.borrow();
        if let Some(callback) = window.get_tooltip_callback() {
            // C++ toolTipWindow->m_tooltip(window, &instData, packedMouseCoords)
            callback(&window, window.instance_data(), packed);
            return;
        }
        let tooltip = window.get_tooltip();
        if !tooltip.is_empty() {
            let delay = window.get_tooltip_delay();
            let tooltip = tooltip.to_string();
            drop(window);
            with_mouse(|mouse| mouse.set_cursor_tooltip(tooltip, Some(delay), None, None));
        }
    }

    pub(crate) fn find_tooltip_window_at_point(
        &self,
        x: i32,
        y: i32,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        if let Some(modal) = &self.modal_stack {
            return Some(self.find_child_at_point_or_self(&modal.window, x, y, true));
        }

        let passes: [fn(WindowStatus) -> bool; 3] = [
            |status| status.contains(WindowStatus::ABOVE),
            |status| !status.intersects(WindowStatus::ABOVE | WindowStatus::BELOW),
            |status| status.contains(WindowStatus::BELOW),
        ];

        for matches_pass in passes {
            for window in &self.root_windows {
                let window_borrow = window.borrow();
                let status = window_borrow.get_status();
                let matches = matches_pass(status);
                let hidden = window_borrow.is_hidden();
                let enabled = window_borrow.is_enabled();
                let contains_point = window_borrow.point_in_window(x, y);
                drop(window_borrow);

                if matches && !hidden && contains_point {
                    let child = self.find_child_at_point_or_self(window, x, y, true);
                    let child_borrow = child.borrow();
                    let has_tooltip = !child_borrow.get_tooltip().is_empty()
                        || child_borrow.get_tooltip_callback().is_some();
                    drop(child_borrow);
                    if has_tooltip || enabled {
                        return Some(child);
                    }
                }
            }
        }

        None
    }

    /// Process key event
    pub fn process_key_event(&mut self, key: u8, state: u8) -> WindowInputReturnCode {
        if key == 0 {
            return WindowInputReturnCode::NotUsed;
        }

        if let Some(mut window) = self.get_focus() {
            loop {
                let result = window.borrow_mut().send_input_message(
                    WindowMessage::Char,
                    key as WindowMsgData,
                    state as WindowMsgData,
                );
                if result.is_handled() {
                    return WindowInputReturnCode::Used;
                }

                let parent = window.borrow().get_parent();
                if let Some(parent) = parent {
                    window = parent;
                } else {
                    return WindowInputReturnCode::NotUsed;
                }
            }
        } else {
            WindowInputReturnCode::NotUsed
        }
    }

    /// Get window under cursor coordinates
    pub fn get_window_under_cursor(
        &self,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        if let Some(capture) = self.get_capture() {
            return Self::filter_window_under_cursor(
                Some(self.find_child_at_point_or_self(&capture, x, y, ignore_enabled)),
                ignore_enabled,
            );
        }

        if let Some(grab_window) = self.get_grab_window() {
            return Self::filter_window_under_cursor(
                Some(self.find_child_at_point_or_self(&grab_window, x, y, ignore_enabled)),
                ignore_enabled,
            );
        }

        // Check modal windows first
        if let Some(modal) = &self.modal_stack {
            return Self::filter_window_under_cursor(
                Some(self.find_child_at_point_or_self(&modal.window, x, y, ignore_enabled)),
                ignore_enabled,
            );
        }

        // Match C++ getWindowUnderCursor: root windows are tested head-first in
        // ABOVE, normal, then BELOW passes so input priority mirrors status.
        let passes: [fn(WindowStatus) -> bool; 3] = [
            |status| status.contains(WindowStatus::ABOVE),
            |status| !status.intersects(WindowStatus::ABOVE | WindowStatus::BELOW),
            |status| status.contains(WindowStatus::BELOW),
        ];

        for matches_pass in passes {
            for window in &self.root_windows {
                if !matches_pass(window.borrow().get_status()) {
                    continue;
                }
                if let Some(found) = self.find_window_at_point_raw(window, x, y, ignore_enabled) {
                    return Self::filter_window_under_cursor(Some(found), ignore_enabled);
                }
            }
        }

        None
    }

    pub(crate) fn filter_window_under_cursor(
        window: Option<Rc<RefCell<GameWindow>>>,
        ignore_enabled: bool,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let window = window?;
        let status = window.borrow().get_status();
        if status.contains(WindowStatus::NO_INPUT)
            || (ignore_enabled && !status.contains(WindowStatus::ENABLED))
        {
            None
        } else {
            Some(window)
        }
    }

    pub(crate) fn get_input_window_under_cursor(
        &self,
        x: i32,
        y: i32,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        if let Some(modal) = &self.modal_stack {
            return self.normalize_input_hit(self.find_window_at_point_raw(
                &modal.window,
                x,
                y,
                false,
            ));
        }

        let passes: [fn(WindowStatus) -> bool; 3] = [
            |status| status.contains(WindowStatus::ABOVE),
            |status| !status.intersects(WindowStatus::ABOVE | WindowStatus::BELOW),
            |status| status.contains(WindowStatus::BELOW),
        ];

        for matches_pass in passes {
            for window in &self.root_windows {
                if !matches_pass(window.borrow().get_status()) {
                    continue;
                }
                if let Some(found) = self.find_window_at_point_raw(window, x, y, false) {
                    return self.normalize_input_hit(Some(found));
                }
            }
        }

        None
    }

    pub(crate) fn normalize_input_hit(
        &self,
        window: Option<Rc<RefCell<GameWindow>>>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let window = window?;
        if !window
            .borrow()
            .get_status()
            .contains(WindowStatus::NO_INPUT)
        {
            return Some(window);
        }

        let parent = window.borrow().get_parent()?;
        if (parent.borrow().get_style() & GWS_COMBO_BOX) != 0 {
            Some(parent)
        } else {
            None
        }
    }

    pub(crate) fn find_child_at_point_or_self(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Rc<RefCell<GameWindow>> {
        GameWindow::point_in_child(window, x, y, ignore_enabled)
    }

    /// Find window at specific point (recursive)
    pub(crate) fn find_window_at_point(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let window_borrow = window.borrow();

        // Skip if hidden or no-input
        if window_borrow.is_hidden() || window_borrow.get_status().contains(WindowStatus::NO_INPUT)
        {
            return None;
        }

        // Skip if disabled (unless ignoring enabled state)
        if !ignore_enabled && !window_borrow.is_enabled() {
            return None;
        }

        // Check if point is in this window
        if window_borrow.point_in_window(x, y) {
            // Check children first (they're on top)
            for child in window_borrow.children() {
                if let Some(found) = self.find_window_at_point(child, x, y, ignore_enabled) {
                    return Some(found);
                }
            }

            // Return this window if no child found
            return Some(window.clone());
        }

        None
    }

    pub(crate) fn find_window_at_point_raw(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let window_borrow = window.borrow();

        if window_borrow.is_hidden() {
            return None;
        }

        if !ignore_enabled && !window_borrow.is_enabled() {
            return None;
        }

        if window_borrow.point_in_window(x, y) {
            for child in window_borrow.children() {
                if let Some(found) = self.find_window_at_point_raw(child, x, y, ignore_enabled) {
                    return Some(found);
                }
            }

            return Some(window.clone());
        }

        None
    }
}
