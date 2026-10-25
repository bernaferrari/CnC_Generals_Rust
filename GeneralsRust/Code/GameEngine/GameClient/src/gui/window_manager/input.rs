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

    fn with_input_effects<R>(
        &mut self,
        context: Option<&mut super::ControlBarInputContext<'_>>,
        effects: Option<&mut crate::gui::game_window::WindowEventEffects<'_>>,
        operation: impl FnOnce(&mut Self, &mut crate::gui::game_window::WindowEventEffects<'_>) -> R,
    ) -> R {
        match (context, effects) {
            (Some(context), None) => context.with_effects(self, operation),
            (None, Some(effects)) => operation(self, effects),
            _ => panic!("window input requires exactly one receiving context"),
        }
    }
    fn send_input_receiving(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        routed: bool,
        context: Option<&mut super::ControlBarInputContext<'_>>,
        effects: Option<&mut crate::gui::game_window::WindowEventEffects<'_>>,
    ) -> WindowMsgHandled {
        match (context, effects) {
            (Some(context), None) => {
                self.send_input_selected(window, msg, data1, data2, routed, context)
            }
            (None, Some(effects)) => self
                .send_input_selected_with_message_sink(window, msg, data1, data2, routed, effects),
            _ => panic!("window input requires exactly one receiving context"),
        }
    }
    pub fn set_lone_window(
        &mut self,
        window: Option<&Rc<RefCell<GameWindow>>>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.set_lone_window_with_selected(window, effects, None)
    }
    /// Set lone window (for exclusive operations like combo boxes)
    pub(crate) fn set_lone_window_with_selected(
        &mut self,
        window: Option<&Rc<RefCell<GameWindow>>>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
        selected: Option<&mut GameWindow>,
    ) {
        const GGM_LEFT_DRAG: u32 = 16384;
        const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;
        if let Some(old) = self.lone_window.as_ref().and_then(|w| w.upgrade()) {
            let same = window.map(|w| Rc::ptr_eq(&old, w)).unwrap_or(false);
            if !same {
                let _ = effects.with_receiving_manager(self, |effects| {
                    super::transition_access::borrow_callback_window(&old, selected)
                        .send_system_message(WindowMessage::User(GGM_CLOSE), 0, 0, effects)
                });
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowInputReturnCode {
        self.process_mouse_event_with_delta(msg, x, y, data, None, effects)
    }

    /// Process mouse event with optional mouse delta for legacy drag handling.
    pub fn process_mouse_event_with_delta(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
        mouse_delta: Option<(i32, i32)>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowInputReturnCode {
        self.process_mouse_event_phase(msg, x, y, data, mouse_delta, None, Some(effects))
    }
    pub(crate) fn process_mouse_event_with_delta_and_message_sink(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
        mouse_delta: Option<(i32, i32)>,
        emitted_messages: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowInputReturnCode {
        self.process_mouse_event_phase(msg, x, y, data, mouse_delta, None, Some(emitted_messages))
    }
    pub fn process_mouse_event_with_control_bar(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
        context: &mut super::ControlBarInputContext<'_>,
    ) -> WindowInputReturnCode {
        self.process_mouse_event_with_delta_and_control_bar(msg, x, y, data, None, context)
    }

    pub(crate) fn process_mouse_event_with_delta_and_control_bar(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
        delta: Option<(i32, i32)>,
        context: &mut super::ControlBarInputContext<'_>,
    ) -> WindowInputReturnCode {
        self.process_mouse_event_phase(msg, x, y, data, delta, Some(context), None)
    }
    fn process_mouse_event_phase(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
        mouse_delta: Option<(i32, i32)>,
        mut context: Option<&mut super::ControlBarInputContext<'_>>,
        mut emitted_messages: Option<&mut crate::gui::game_window::WindowEventEffects<'_>>,
    ) -> WindowInputReturnCode {
        const GGM_LEFT_DRAG: u32 = 16384;
        const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;
        let old_lone = self.lone_window.as_ref().and_then(|w| w.upgrade());
        self.update_cursor_tooltip_for_mouse_event_owned(x, y, context.as_deref_mut());
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
                            let _ = if let Some(emitted) = emitted_messages.as_deref_mut() {
                                self.send_input_selected_with_message_sink(
                                    &grab_window,
                                    msg,
                                    data,
                                    0,
                                    false,
                                    emitted,
                                )
                            } else {
                                self.send_input_selected(
                                    &grab_window,
                                    msg,
                                    data,
                                    0,
                                    false,
                                    context
                                        .as_deref_mut()
                                        .expect("native input requires receiving context"),
                                )
                            };
                        }
                        self.set_grab_window(None);
                        return WindowInputReturnCode::Used;
                    }
                    WindowMessage::None | WindowMessage::LeftDrag => {
                        if let Some((dx, dy)) = mouse_delta {
                            self.move_grabbed_draggable_window(&grab_window, dx, dy);
                        }
                        let _ = if let Some(emitted) = emitted_messages.as_deref_mut() {
                            self.send_input_selected_with_message_sink(
                                &grab_window,
                                msg,
                                data,
                                0,
                                false,
                                emitted,
                            )
                        } else {
                            self.send_input_selected(
                                &grab_window,
                                msg,
                                data,
                                0,
                                false,
                                context
                                    .as_deref_mut()
                                    .expect("native input requires receiving context"),
                            )
                        };
                        return WindowInputReturnCode::Used;
                    }
                    _ => {
                        return WindowInputReturnCode::Used;
                    }
                }
            }
        }

        if let Some(window) = target_window {
            let handled_window = self.send_mouse_message_up_chain_phase(
                &window,
                msg,
                data,
                x,
                y,
                capture_window.as_ref(),
                context.as_deref_mut(),
                emitted_messages.as_deref_mut(),
            );

            self.update_current_mouse_region(
                Some(&window),
                capture_window.as_ref(),
                x,
                y,
                context.as_deref_mut(),
                emitted_messages.as_deref_mut(),
            );

            self.close_lone_window_after_mouse_message(
                old_lone.as_ref(),
                handled_window.as_ref(),
                msg,
                context.as_deref_mut(),
                emitted_messages.as_deref_mut(),
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
            self.update_current_mouse_region(
                None,
                capture_window.as_ref(),
                x,
                y,
                context.as_deref_mut(),
                emitted_messages.as_deref_mut(),
            );
            if matches!(
                msg,
                WindowMessage::LeftUp | WindowMessage::MiddleUp | WindowMessage::RightUp
            ) {
                self.close_lone_window_after_mouse_message(
                    old_lone.as_ref(),
                    None,
                    msg,
                    context.as_deref_mut(),
                    emitted_messages.as_deref_mut(),
                );
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
        mut context: Option<&mut super::ControlBarInputContext<'_>>,
        mut emitted_messages: Option<&mut crate::gui::game_window::WindowEventEffects<'_>>,
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

        // C++ GameWindowManager.cpp:1228-1236 forwards the same packed
        // screen coordinates as the raw event, including negative positions.
        let packed = (((y as u32) << 16) | ((x as u32) & 0xFFFF)) as WindowMsgData;
        if let Some(prev) = previous {
            let should_send_leaving = capture_window
                .map(|capture| Self::is_strict_descendant(capture, &prev))
                .unwrap_or(true);
            if should_send_leaving {
                let (px, py) = prev.borrow().get_screen_position();
                let _ = prev.borrow_mut().set_cursor_position(x - px, y - py);
                let _ = self.send_input_receiving(
                    &prev,
                    WindowMessage::MouseLeaving,
                    packed,
                    0,
                    true,
                    context.as_deref_mut(),
                    emitted_messages.as_deref_mut(),
                );
            }
        }

        if let Some(new_window) = new_window {
            let (wx, wy) = new_window.borrow().get_screen_position();
            let _ = new_window.borrow_mut().set_cursor_position(x - wx, y - wy);
            let _ = self.send_input_receiving(
                new_window,
                WindowMessage::MouseEntering,
                packed,
                0,
                true,
                context.as_deref_mut(),
                emitted_messages.as_deref_mut(),
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
        mut context: Option<&mut super::ControlBarInputContext<'_>>,
        mut emitted_messages: Option<&mut crate::gui::game_window::WindowEventEffects<'_>>,
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

        self.with_input_effects(context, emitted_messages, |manager, effects| {
            manager.set_lone_window(None, effects)
        });
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        self.send_mouse_message_up_chain_phase(start, msg, data, x, y, stop_at, None, Some(effects))
    }
    fn send_mouse_message_up_chain_phase(
        &mut self,
        start: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data: WindowMsgData,
        x: i32,
        y: i32,
        stop_at: Option<&Rc<RefCell<GameWindow>>>,
        mut context: Option<&mut super::ControlBarInputContext<'_>>,
        mut emitted_messages: Option<&mut crate::gui::game_window::WindowEventEffects<'_>>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let mut current = Some(start.clone());
        while let Some(window) = current {
            let (wx, wy) = window.borrow().get_screen_position();
            {
                let _ = window.borrow_mut().set_cursor_position(x - wx, y - wy);
            }
            let result = if let Some(emitted) = emitted_messages.as_deref_mut() {
                self.send_input_selected_with_message_sink(
                    &window,
                    msg,
                    data,
                    0,
                    stop_at.is_some(),
                    emitted,
                )
            } else {
                self.send_input_selected(
                    &window,
                    msg,
                    data,
                    0,
                    stop_at.is_some(),
                    context
                        .as_deref_mut()
                        .expect("native input requires receiving context"),
                )
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

    /// Native per-frame hover observation shares the same hit-test and formatter
    /// as delivered input, without delivering another gadget input event.
    pub fn refresh_control_bar_tooltip(
        &mut self,
        x: i32,
        y: i32,
        context: &mut super::ControlBarInputContext<'_>,
    ) {
        self.update_cursor_tooltip_for_mouse_event_owned(x, y, Some(context));
    }

    fn update_cursor_tooltip_for_mouse_event_owned(
        &mut self,
        x: i32,
        y: i32,
        context: Option<&mut super::ControlBarInputContext<'_>>,
    ) {
        let Some(context) = context else {
            return self.update_cursor_tooltip_for_mouse_event(x, y);
        };
        // The source clears the previous generic tooltip before hit-testing,
        // including capture, disabled tooltips and an owned command callback.
        if let Some(mouse) = context.cursor_mouse {
            mouse
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .set_cursor_tooltip(String::new(), None, None, None);
        }
        if self.get_capture().is_some() || self.get_grab_window().is_some() {
            return;
        }
        let Some(window) = self.find_tooltip_window_at_point(x, y) else {
            return;
        };
        if window.borrow().has_owned_control_bar_tooltip() {
            // Release the child before layout creation/population uses its manager.
            if !context.tooltips_disabled && !context.game_ending {
                let upgrade_source = context.upgrade_tooltip;
                context.with_tooltip_effects(self, |tooltip, manager, effects| {
                    crate::gui::gui_callbacks::control_bar_popup_description::show_build_tooltip_layout_with_upgrade_source(
                        tooltip, manager, window, upgrade_source, effects,
                    );
                });
            }
            return;
        }
        let packed = ((y as u32) << 16) | ((x as u32) & 0xffff);
        let window = window.borrow();
        if let Some(callback) = window.get_tooltip_callback() {
            callback(&window, window.instance_data(), packed);
        } else if !window.get_tooltip().is_empty() {
            if let Some(mouse) = context.cursor_mouse {
                mouse
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .set_cursor_tooltip(
                        window.get_tooltip().to_string(),
                        Some(window.get_tooltip_delay()),
                        None,
                        None,
                    );
            }
        }
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
                        || child_borrow.has_owned_control_bar_tooltip()
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
    pub fn process_key_event(
        &mut self,
        key: u8,
        state: u8,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowInputReturnCode {
        self.process_key_event_with_message_sink(key, state, effects)
    }
    pub(crate) fn process_key_event_with_message_sink(
        &mut self,
        key: u8,
        state: u8,
        emitted_messages: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowInputReturnCode {
        self.process_key_event_phase(
            key,
            state,
            &mut |manager, window, message, data1, data2, routed| {
                manager.send_input_selected_with_message_sink(
                    window,
                    message,
                    data1,
                    data2,
                    routed,
                    emitted_messages,
                )
            },
        )
    }
    pub fn process_key_event_with_control_bar(
        &mut self,
        key: u8,
        state: u8,
        context: &mut super::ControlBarInputContext<'_>,
    ) -> WindowInputReturnCode {
        self.process_key_event_phase(
            key,
            state,
            &mut |manager, window, message, data1, data2, routed| {
                manager.send_input_selected(window, message, data1, data2, routed, &mut *context)
            },
        )
    }
    /// Same focus/ancestor selection, with one short receiver invoked at every
    /// original delivery point. The receiver finishes synchronous continuation
    /// before this kernel selects or delivers the next ancestor callback.
    pub(crate) fn process_key_event_with_receiving_sender(
        &mut self,
        key: u8,
        state: u8,
        receive: &mut dyn FnMut(
            &mut Self,
            &Rc<RefCell<GameWindow>>,
            WindowMessage,
            WindowMsgData,
            WindowMsgData,
            bool,
        ) -> WindowMsgHandled,
    ) -> WindowInputReturnCode {
        self.process_key_event_phase(key, state, receive)
    }
    fn process_key_event_phase(
        &mut self,
        key: u8,
        state: u8,
        receive: &mut dyn FnMut(
            &mut Self,
            &Rc<RefCell<GameWindow>>,
            WindowMessage,
            WindowMsgData,
            WindowMsgData,
            bool,
        ) -> WindowMsgHandled,
    ) -> WindowInputReturnCode {
        if key == 0 {
            return WindowInputReturnCode::NotUsed;
        }

        if let Some(mut window) = self.get_focus() {
            loop {
                let result = receive(
                    self,
                    &window,
                    WindowMessage::Char,
                    key as WindowMsgData,
                    state as WindowMsgData,
                    false,
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
