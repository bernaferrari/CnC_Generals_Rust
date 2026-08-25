//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use super::prelude::*;

impl GameWindow {
    /// Create a new GameWindow
    pub fn new() -> Self {
        Self {
            id: WINDOW_ID_INVALID,
            status: WindowStatus::NONE,
            size: Point2D { x: 0, y: 0 },
            region: WindowRegion::default(),
            cursor_pos: Cell::new(Point2D { x: 0, y: 0 }),
            inst_data: WindowInstanceData::default(),
            user_data: None,
            edit_data: None,
            parent: None,
            children: Vec::new(),
            next_sibling: None,
            prev_sibling: None,
            owner_is_self: false,
            next_in_layout: None,
            prev_in_layout: None,
            layout: None,
            callbacks: WindowCallbacks {
                draw: Some(Box::new(legacy_default_draw_callback)),
                tooltip: None,
                input: Some(Box::new(default_input_callback)),
                system: Some(Box::new(default_system_callback)),
            },
            widget: None,
            combobox_links: None,
            listbox_links: None,
            slider_thumb: None,
            press_scale: 1.0,
            press_scale_target: 1.0,
            press_scale_velocity: 0.0,
            press_spring_strength: 60.0,
            press_spring_damping: 10.0,
            press_impulse: -4.5,
            release_impulse: 5.5,
            press_was_down: false,
        }
    }

    /// Get window ID
    pub fn get_id(&self) -> WindowId {
        self.id
    }

    /// Get window style flags
    pub fn get_style(&self) -> u32 {
        self.inst_data.style
    }

    pub(crate) fn is_press_anim_enabled(&self) -> bool {
        if matches!(
            self.widget,
            Some(WindowWidget::PushButton(_))
                | Some(WindowWidget::CheckBox(_))
                | Some(WindowWidget::RadioButton(_))
        ) {
            return true;
        }
        self.inst_data.style & (GWS_PUSH_BUTTON | GWS_CHECK_BOX | GWS_RADIO_BUTTON) != 0
    }

    pub fn get_press_scale(&self) -> f32 {
        if self.is_press_anim_enabled() {
            self.press_scale
        } else {
            1.0
        }
    }

    pub(crate) fn sync_state_from_widget(&mut self) {
        let (pressed, hilited, selected, has_widget) = if let Some(widget) = self.widget.as_ref() {
            let widget_state = widget.state();
            let selected = match widget {
                WindowWidget::CheckBox(checkbox) => Some(checkbox.is_checked()),
                WindowWidget::RadioButton(radio) => Some(radio.is_selected()),
                _ => None,
            };
            (
                matches!(widget_state, GadgetState::Pressed),
                matches!(widget_state, GadgetState::Hovered | GadgetState::Pressed),
                selected,
                true,
            )
        } else {
            let pressed = self.inst_data.state.contains(WindowState::PUSHED);
            let hilited = self.inst_data.state.contains(WindowState::HILITED) || pressed;
            (pressed, hilited, None, false)
        };

        if has_widget {
            let mut state = self.inst_data.state;
            state.remove(WindowState::HILITED | WindowState::PUSHED);
            if hilited {
                state.insert(WindowState::HILITED);
            }
            if pressed {
                state.insert(WindowState::PUSHED);
            }
            if let Some(selected) = selected {
                state.set(WindowState::SELECTED, selected);
            }
            self.inst_data.state = state;
        }

        if self.is_press_anim_enabled() && pressed != self.press_was_down {
            self.press_scale_target = if pressed { 0.94 } else { 1.0 };
            self.press_scale_velocity = if pressed {
                self.press_impulse
            } else {
                self.release_impulse
            };
            self.press_was_down = pressed;
        }
    }

    pub fn update_press_animation(&mut self, delta_time: f32) {
        if !self.is_press_anim_enabled() {
            self.press_scale = 1.0;
            self.press_scale_target = 1.0;
            self.press_scale_velocity = 0.0;
            self.press_was_down = false;
            return;
        }

        // Keep press animation in sync even if input bypassed window message routing.
        self.sync_state_from_widget();

        let dt = delta_time.max(0.0);
        if dt == 0.0 {
            return;
        }

        let displacement = self.press_scale - self.press_scale_target;
        let accel = -self.press_spring_strength * displacement
            - self.press_spring_damping * self.press_scale_velocity;
        self.press_scale_velocity += accel * dt;
        self.press_scale += self.press_scale_velocity * dt;

        if (self.press_scale - self.press_scale_target).abs() < 0.0005
            && self.press_scale_velocity.abs() < 0.0005
        {
            self.press_scale = self.press_scale_target;
            self.press_scale_velocity = 0.0;
        }
    }

    /// Get tooltip delay in milliseconds
    pub fn get_tooltip_delay(&self) -> i32 {
        self.inst_data.tooltip_delay
    }

    /// Set window ID
    pub fn set_id(&mut self, id: WindowId) {
        self.id = id;
        self.inst_data.id = id;
    }

    /// Get window size
    pub fn get_size(&self) -> (i32, i32) {
        (self.size.x, self.size.y)
    }

    /// Set window size
    pub fn set_size(&mut self, width: i32, height: i32) -> WindowResult<()> {
        self.size.x = width;
        self.size.y = height;
        self.region.high.x = self.region.low.x + width;
        self.region.high.y = self.region.low.y + height;
        let _ = self.send_system_message(
            WindowMessage::User(GGM_RESIZED),
            width as WindowMsgData,
            height as WindowMsgData,
        );
        let mut resize_tab_panes = false;
        let mut sync_listbox_inset = false;
        match self.widget.as_mut() {
            Some(WindowWidget::ListBox(listbox)) => {
                listbox.set_size(width.max(0) as u32, height.max(0) as u32);
                sync_listbox_inset = true;
            }
            Some(WindowWidget::TabControl(tab_control)) => {
                tab_control.set_size(width.max(0) as u32, height.max(0) as u32);
                resize_tab_panes = true;
            }
            _ => {}
        }
        if resize_tab_panes {
            self.resize_tab_panes_to_content();
        }
        if sync_listbox_inset {
            self.sync_listbox_content_top_inset();
        }
        if self.slider_thumb.is_some() {
            self.update_slider_thumb();
        }
        if let Some(links) = self.combobox_links {
            let button_width = 21;
            let base_height = if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box.borrow().get_size().1
            } else {
                height
            };
            if let Some(drop_down) = self.find_child_by_id(links.drop_down) {
                let _ = drop_down
                    .borrow_mut()
                    .set_position((width - button_width).max(0), 0);
                let _ = drop_down.borrow_mut().set_size(button_width, base_height);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                let _ = edit_box.borrow_mut().set_position(0, 0);
                let _ = edit_box
                    .borrow_mut()
                    .set_size((width - button_width).max(0), base_height);
            }
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                let current_list_height = list_box.borrow().get_size().1;
                let list_height = if height > base_height {
                    height - base_height
                } else {
                    current_list_height
                };
                let _ = list_box.borrow_mut().set_position(0, base_height);
                let _ = list_box.borrow_mut().set_size(width, list_height);
            }
        }
        if let Some(links) = self.listbox_links {
            let button_width = 21;
            let button_height = 22;
            let has_title = !self.inst_data.text.is_empty();
            let font_height = if has_title {
                with_window_manager_ref(|manager| {
                    self.inst_data
                        .font
                        .as_ref()
                        .map(|font| manager.win_font_height(font))
                        .unwrap_or(12)
                })
            } else {
                0
            };
            let top = if has_title { font_height + 1 } else { 0 };
            let bottom = if has_title {
                height - (font_height + 1)
            } else {
                height
            };

            if let Some(up_button) = self.find_child_by_id(links.up_button) {
                let _ = up_button
                    .borrow_mut()
                    .set_position(width - button_width - 2, top + 2);
                let _ = up_button.borrow_mut().set_size(button_width, button_height);
            }
            if let Some(down_button) = self.find_child_by_id(links.down_button) {
                let _ = down_button
                    .borrow_mut()
                    .set_position(width - button_width - 2, top + bottom - button_height - 2);
                let _ = down_button
                    .borrow_mut()
                    .set_size(button_width, button_height);
            }
            if let Some(slider) = self.find_child_by_id(links.slider) {
                let slider_height = (bottom - (2 * button_height) - 6).max(0);
                let _ = slider
                    .borrow_mut()
                    .set_position(width - button_width - 2, top + button_height + 3);
                let _ = slider.borrow_mut().set_size(button_width, slider_height);
            }
            if let Some(thumb_id) = links.thumb {
                if let Some(thumb) = self.find_child_by_id(thumb_id) {
                    let _ = thumb.borrow_mut().set_size(button_width, 16);
                }
            }
            self.update_listbox_scrollbar();
        }
        Ok(())
    }

    /// Get window position
    pub fn get_position(&self) -> (i32, i32) {
        (self.region.low.x, self.region.low.y)
    }

    /// Set window position
    pub fn set_position(&mut self, x: i32, y: i32) -> WindowResult<()> {
        self.region.low.x = x;
        self.region.low.y = y;
        self.region.high.x = x + self.size.x;
        self.region.high.y = y + self.size.y;
        self.normalize_region();
        Ok(())
    }

    /// Get screen position (including parent offsets)
    pub fn get_screen_position(&self) -> (i32, i32) {
        let mut x = self.region.low.x;
        let mut y = self.region.low.y;

        let mut current_parent = self.parent.as_ref().and_then(|w| w.upgrade());
        while let Some(parent_rc) = current_parent.take() {
            if let Ok(parent) = parent_rc.try_borrow() {
                x += parent.region.low.x;
                y += parent.region.low.y;
                current_parent = parent.parent.as_ref().and_then(|w| w.upgrade());
            } else {
                // Parent is already borrowed on this thread. Fail-closed: stop
                // walking rather than aliasing the RefCell.
                break;
            }
        }

        (x, y)
    }

    /// Get window region
    pub fn get_region(&self) -> WindowRegion {
        self.region
    }

    /// Set cursor position within window
    pub fn set_cursor_position(&mut self, x: i32, y: i32) -> WindowResult<()> {
        self.set_cursor_position_from_draw(x, y);
        Ok(())
    }

    pub(crate) fn set_cursor_position_from_draw(&self, x: i32, y: i32) {
        self.cursor_pos.set(Point2D { x, y });
    }

    /// Get cursor position within window
    pub fn get_cursor_position(&self) -> (i32, i32) {
        let cursor = self.cursor_pos.get();
        (cursor.x, cursor.y)
    }

    /// Check if point is within window (including children)
    pub fn point_in_window(&self, x: i32, y: i32) -> bool {
        let (win_x, win_y) = self.get_screen_position();
        let (width, height) = self.get_size();

        x >= win_x && x <= win_x + width && y >= win_y && y <= win_y + height
    }

    /// Return the deepest enabled, visible child at a point, or the given window.
    pub fn point_in_child(
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Rc<RefCell<GameWindow>> {
        Self::point_in_child_ex(window, x, y, ignore_enabled, false)
    }

    /// C++ `GameWindow::winPointInChild(..., playDisabledSound)`.
    pub fn point_in_child_ex(
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_enabled: bool,
        play_disabled_sound: bool,
    ) -> Rc<RefCell<GameWindow>> {
        let children = window.borrow().children().to_vec();
        for child in children {
            let child_borrow = child.borrow();
            let contains_point = child_borrow.point_in_window(x, y);
            let hidden = child_borrow.is_hidden();
            let enabled =
                ignore_enabled || child_borrow.get_status().contains(WindowStatus::ENABLED);
            drop(child_borrow);

            if contains_point && !hidden {
                if enabled {
                    return Self::point_in_child_ex(
                        &child,
                        x,
                        y,
                        ignore_enabled,
                        play_disabled_sound,
                    );
                } else if play_disabled_sound {
                    if let Some(audio) = gamelogic::helpers::TheAudio::get() {
                        let event =
                            gamelogic::common::audio::AudioEventRts::new("GUIClickDisabled");
                        audio.add_audio_event(&event);
                    }
                }
            }
        }

        window.clone()
    }

    /// Return the child at a point regardless of enabled state, optionally skipping hidden children.
    pub fn point_in_any_child(
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_hidden: bool,
        ignore_enabled: bool,
    ) -> Rc<RefCell<GameWindow>> {
        let children = window.borrow().children().to_vec();
        for child in children {
            let child_borrow = child.borrow();
            let contains_point = child_borrow.point_in_window(x, y);
            let skip_hidden = ignore_hidden && child_borrow.is_hidden();
            drop(child_borrow);

            if contains_point && !skip_hidden {
                return Self::point_in_child(&child, x, y, ignore_enabled);
            }
        }

        window.clone()
    }

    /// Get window status flags
    pub fn get_status(&self) -> WindowStatus {
        self.status
    }

    /// Set window status flags
    pub fn set_status(&mut self, status: WindowStatus) -> WindowStatus {
        let old_status = self.status;
        self.status |= status;
        self.inst_data.status = self.status;
        old_status
    }

    /// Clear window status flags
    pub fn clear_status(&mut self, status: WindowStatus) -> WindowStatus {
        let old_status = self.status;
        self.status &= !status;
        self.inst_data.status = self.status;
        old_status
    }

    /// Enable or disable the window
    pub fn enable(&mut self, enable: bool) -> WindowResult<()> {
        if enable {
            self.status |= WindowStatus::ENABLED;
        } else {
            self.status &= !WindowStatus::ENABLED;
        }
        self.inst_data.status = self.status;
        if let Some(widget) = &mut self.widget {
            widget.set_enabled(enable);
        }

        // Enable/disable all children. Nested create/focus/hide callbacks may
        // already hold a child RefCell — queue instead of panic or fail-closed.
        let children = self.children.clone();
        for child_rc in children {
            let queued = child_rc.try_borrow_mut().is_err();
            if queued {
                queue_window_manager_op(move |_manager| {
                    if let Ok(mut child) = child_rc.try_borrow_mut() {
                        let _ = child.enable(enable);
                    } else {
                        let child_rc = child_rc.clone();
                        crate::gui::window_manager::queue_window_manager_op_deferred(
                            move |_manager| {
                                if let Ok(mut child) = child_rc.try_borrow_mut() {
                                    let _ = child.enable(enable);
                                }
                            },
                        );
                    }
                });
            } else {
                child_rc.borrow_mut().enable(enable)?;
            }
        }

        Ok(())
    }

    /// Check if window is enabled (C++ parity: checks all parents too)
    pub fn is_enabled(&self) -> bool {
        if !self.status.contains(WindowStatus::ENABLED) {
            return false;
        }
        // C++ parity: isEnabled() walks up parent chain
        let mut current = self.parent.as_ref().and_then(|w| w.upgrade());
        while let Some(parent_rc) = current {
            if let Ok(parent) = parent_rc.try_borrow() {
                if !parent.status.contains(WindowStatus::ENABLED) {
                    return false;
                }
                current = parent.parent.as_ref().and_then(|w| w.upgrade());
            } else {
                // Parent is already borrowed on this thread. Fail-closed: treat
                // as not enabled rather than aliasing the RefCell.
                return false;
            }
        }
        true
    }

    pub fn hide(&mut self, hide: bool) -> WindowResult<()> {
        self.set_hidden_status(hide);
        if hide {
            let window_ptr = self as *const GameWindow;
            let children = self.children.clone();
            // C++ winHide is not re-entrant. Never enqueue into the in-flight
            // drain (that loops forever when MainMenuUpdate already holds WM).
            queue_window_manager_op_deferred(move |manager| {
                manager.window_hiding_from_direct_hide(window_ptr, children);
            });
        }
        Ok(())
    }

    pub(crate) fn hide_without_manager_side_effects(&mut self, hide: bool) -> WindowResult<()> {
        self.set_hidden_status(hide);
        Ok(())
    }

    pub(crate) fn set_hidden_status(&mut self, hide: bool) {
        if hide {
            // C++ parity: parent visibility suppresses child rendering/input through
            // ancestry checks in is_hidden(), rather than permanently mutating every
            // child hidden bit when the parent is toggled.
            self.status |= WindowStatus::HIDDEN;
            self.inst_data.status = self.status;
            if let Some(widget) = &mut self.widget {
                widget.set_visible(false);
            }
        } else {
            self.status &= !WindowStatus::HIDDEN;
            self.inst_data.status = self.status;
            if let Some(widget) = &mut self.widget {
                widget.set_visible(true);
            }
        }
    }

    /// Check if this window's own hidden bit is set.
    pub fn is_hidden(&self) -> bool {
        self.status.contains(WindowStatus::HIDDEN)
    }

    /// Activate the window (bring to front and show)
    pub fn activate(&mut self) -> WindowResult<()> {
        self.status |= WindowStatus::ACTIVE;
        self.inst_data.status = self.status;
        self.hide(false)?;
        Ok(())
    }

    /// Set window text
    pub fn set_text(&mut self, text: &str) -> WindowResult<()> {
        self.inst_data.text = text.to_string();
        if let Some(widget) = self.widget.as_mut() {
            match widget {
                WindowWidget::PushButton(button) => button.set_text(text),
                WindowWidget::RadioButton(radio) => radio.set_label(text),
                WindowWidget::CheckBox(checkbox) => checkbox.set_label(text),
                WindowWidget::StaticText(label) => label.set_text(text),
                WindowWidget::TextEntry(entry) => entry.set_text(text),
                WindowWidget::ProgressBar(bar) => bar.set_text(text),
                _ => {}
            }
        }
        if let Some(display) = self.ensure_display_text() {
            display.borrow_mut().set_text(text.to_string());
        }
        self.sync_listbox_content_top_inset();
        Ok(())
    }

    /// Get window text
    pub fn get_text(&self) -> &str {
        &self.inst_data.text
    }

    /// Get the number of characters in the window text.
    pub fn get_text_length(&self) -> usize {
        self.inst_data.text.chars().count()
    }

    pub fn get_text_label(&self) -> &str {
        &self.inst_data.text_label
    }

    /// Set tooltip text
    pub fn set_tooltip(&mut self, tooltip: &str) {
        self.inst_data.tooltip = tooltip.chars().take(TOOLTIP_MAX_LEN).collect();
        if let Some(widget) = self.widget.as_mut() {
            if let WindowWidget::ListBox(listbox) = widget {
                listbox.set_tooltip(self.inst_data.tooltip.clone());
            }
        }
        if let Some(display) = self.ensure_display_tooltip() {
            display
                .borrow_mut()
                .set_text(self.inst_data.tooltip.clone());
        }
    }

    /// Get tooltip text
    pub fn get_tooltip(&self) -> &str {
        &self.inst_data.tooltip
    }

    /// Set window font
    pub fn set_font(&mut self, font: GameFont) {
        let font_for_children = font.clone();
        self.inst_data.font = Some(font);
        if let Some(font_desc) = self.inst_data.font.as_ref().map(GameFont::to_font_desc) {
            if let Ok(font_ref) = get_font_library().get_font(&font_desc) {
                if let Some(display) = self.inst_data.display_text.as_ref() {
                    display.borrow_mut().set_font(font_ref.clone());
                }
                if let Some(display) = self.inst_data.display_tooltip.as_ref() {
                    display.borrow_mut().set_font(font_ref);
                }
            }
        }
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box.borrow_mut().set_font(font_for_children.clone());
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box.borrow_mut().set_font(font_for_children);
            }
        }
        self.sync_listbox_content_top_inset();
    }

    /// Get window font
    pub fn get_font(&self) -> Option<&GameFont> {
        self.inst_data.font.as_ref()
    }

    /// Set highlight state
    pub fn set_hilite_state(&mut self, state: bool) {
        if state {
            self.inst_data.state |= WindowState::HILITED;
        } else {
            self.inst_data.state &= !WindowState::HILITED;
        }
    }

    /// Set draw offset for images
    pub fn set_draw_offset(&mut self, x: i32, y: i32) {
        self.inst_data.image_offset.x = x;
        self.inst_data.image_offset.y = y;
    }

    /// Get draw offset
    pub fn get_draw_offset(&self) -> (i32, i32) {
        (self.inst_data.image_offset.x, self.inst_data.image_offset.y)
    }

    /// Set enabled image for draw data at index
    pub fn set_enabled_image(&mut self, index: usize, image: Image) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.enabled_draw_data[index].image = Some(image);
        Ok(())
    }

    /// Get enabled draw data for the specified index.
    pub fn get_enabled_draw_data(&self, index: usize) -> Option<WindowDrawData> {
        if index >= MAX_DRAW_DATA {
            return None;
        }
        Some(self.inst_data.enabled_draw_data[index].clone())
    }

    /// Get disabled draw data for the specified index.
    pub fn get_disabled_draw_data(&self, index: usize) -> Option<WindowDrawData> {
        if index >= MAX_DRAW_DATA {
            return None;
        }
        Some(self.inst_data.disabled_draw_data[index].clone())
    }

    /// Get hilite draw data for the specified index.
    pub fn get_hilite_draw_data(&self, index: usize) -> Option<WindowDrawData> {
        if index >= MAX_DRAW_DATA {
            return None;
        }
        Some(self.inst_data.hilite_draw_data[index].clone())
    }

    /// Get the enabled text color.
    pub fn get_enabled_text_color(&self) -> Color {
        self.inst_data.enabled_text.color
    }

    /// Get the enabled text border color.
    pub fn get_enabled_text_border_color(&self) -> Color {
        self.inst_data.enabled_text.border_color
    }

    /// Get the disabled text color.
    pub fn get_disabled_text_color(&self) -> Color {
        self.inst_data.disabled_text.color
    }

    /// Get the disabled text border color.
    pub fn get_disabled_text_border_color(&self) -> Color {
        self.inst_data.disabled_text.border_color
    }

    /// Get the IME composite text color.
    pub fn get_ime_composite_text_color(&self) -> Color {
        self.inst_data.ime_composite_text.color
    }

    /// Get the IME composite text border color.
    pub fn get_ime_composite_text_border_color(&self) -> Color {
        self.inst_data.ime_composite_text.border_color
    }

    /// Get the hilite text color.
    pub fn get_hilite_text_color(&self) -> Color {
        self.inst_data.hilite_text.color
    }

    /// Get the hilite text border color.
    pub fn get_hilite_text_border_color(&self) -> Color {
        self.inst_data.hilite_text.border_color
    }

    /// Show the window by clearing the hidden flag.
    pub fn show(&mut self) -> WindowResult<()> {
        self.hide(false)
    }

    /// Bring the window to the front of the z-order.
    pub fn bring_to_front(&mut self) -> WindowResult<()> {
        self.status |= WindowStatus::ACTIVE;
        Ok(())
    }

    /// Find a child control by name.
    pub fn find_child<T>(&self, _name: &str) -> Option<T> {
        None
    }

    /// Find a child window by its decorated name.
    pub fn find_child_window(&self, name: &str) -> Option<Rc<RefCell<GameWindow>>> {
        if self.inst_data.decorated_name.eq_ignore_ascii_case(name) {
            if let Some(parent) = self.get_parent() {
                for child_rc in parent.borrow().children() {
                    let child = child_rc.borrow();
                    if child.inst_data.decorated_name.eq_ignore_ascii_case(name) {
                        return Some(child_rc.clone());
                    }
                }
            }
        }
        for child_rc in &self.children {
            let child = child_rc.borrow();
            if child.inst_data.decorated_name.eq_ignore_ascii_case(name) {
                return Some(child_rc.clone());
            }
            if let Some(found) = child.find_child_window(name) {
                return Some(found);
            }
        }
        None
    }

    /// Find a child window by window id.
    pub fn find_child_by_id(&self, id: WindowId) -> Option<Rc<RefCell<GameWindow>>> {
        for child_rc in &self.children {
            let child = child_rc.borrow();
            if child.id == id {
                return Some(child_rc.clone());
            }
            if let Some(found) = child.find_child_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    /// Set enabled color for draw data at index
    pub fn set_enabled_color(&mut self, index: usize, color: Color) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.enabled_draw_data[index].color = color;
        Ok(())
    }

    pub fn set_enabled_border_color(&mut self, index: usize, color: Color) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.enabled_draw_data[index].border_color = color;
        Ok(())
    }

    pub fn set_enabled_draw_colors(
        &mut self,
        index: usize,
        color: Color,
        border_color: Color,
    ) -> WindowResult<()> {
        self.set_enabled_color(index, color)?;
        self.set_enabled_border_color(index, border_color)
    }

    pub fn set_disabled_color(&mut self, index: usize, color: Color) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.disabled_draw_data[index].color = color;
        Ok(())
    }

    pub fn set_disabled_border_color(&mut self, index: usize, color: Color) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.disabled_draw_data[index].border_color = color;
        Ok(())
    }

    pub fn set_disabled_draw_colors(
        &mut self,
        index: usize,
        color: Color,
        border_color: Color,
    ) -> WindowResult<()> {
        self.set_disabled_color(index, color)?;
        self.set_disabled_border_color(index, border_color)
    }

    pub fn set_hilite_color(&mut self, index: usize, color: Color) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.hilite_draw_data[index].color = color;
        Ok(())
    }

    pub fn set_hilite_border_color(&mut self, index: usize, color: Color) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.hilite_draw_data[index].border_color = color;
        Ok(())
    }

    pub fn set_hilite_draw_colors(
        &mut self,
        index: usize,
        color: Color,
        border_color: Color,
    ) -> WindowResult<()> {
        self.set_hilite_color(index, color)?;
        self.set_hilite_border_color(index, border_color)
    }

    /// Set text colors for enabled state
    pub fn set_enabled_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.enabled_text.color = color;
        self.inst_data.enabled_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_enabled_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_enabled_text_colors(color, border_color);
            }
        }
    }

    /// Set text colors for disabled state
    pub fn set_disabled_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.disabled_text.color = color;
        self.inst_data.disabled_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_disabled_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_disabled_text_colors(color, border_color);
            }
        }
    }

    /// Set text colors for hilite state
    pub fn set_hilite_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.hilite_text.color = color;
        self.inst_data.hilite_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_hilite_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_hilite_text_colors(color, border_color);
            }
        }
    }

    /// Set text colors for IME composite state
    pub fn set_ime_composite_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.ime_composite_text.color = color;
        self.inst_data.ime_composite_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_ime_composite_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_ime_composite_text_colors(color, border_color);
            }
        }
    }

    /// Get parent window
    pub fn get_parent(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.parent.as_ref()?.upgrade()
    }

    /// Set parent window.
    pub fn set_parent(&mut self, parent: Option<&Rc<RefCell<GameWindow>>>) {
        self.parent = parent.map(Rc::downgrade);
    }

    /// Get the window that receives gadget notifications from this window.
    pub fn get_owner(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.inst_data.owner.as_ref()?.upgrade()
    }

    /// Set the window that receives gadget notifications from this window.
    pub fn set_owner(&mut self, owner: Option<&Rc<RefCell<GameWindow>>>) {
        self.inst_data.owner = owner.map(Rc::downgrade);
        self.owner_is_self = false;
    }

    /// Set the window owner to this window, matching C++ winSetOwner(NULL).
    pub(crate) fn set_owner_self(&mut self, self_window: &Rc<RefCell<GameWindow>>) {
        self.inst_data.owner = Some(Rc::downgrade(self_window));
        self.owner_is_self = true;
    }

    /// Return whether this window's owner is itself.
    pub fn owner_is_self(&self) -> bool {
        self.owner_is_self
    }

    /// Set the layout this window belongs to.
    pub fn set_layout(&mut self, layout: Option<&Rc<RefCell<crate::gui::WindowLayout>>>) {
        self.layout = layout.map(Rc::downgrade);
    }

    /// Get the layout this window belongs to.
    pub fn get_layout(&self) -> Option<Rc<RefCell<crate::gui::WindowLayout>>> {
        self.layout.as_ref()?.upgrade()
    }

    /// Set the next window in this window's owning layout list.
    pub(crate) fn set_next_in_layout(&mut self, next: Option<&Rc<RefCell<GameWindow>>>) {
        self.next_in_layout = next.map(Rc::downgrade);
    }

    /// Set the previous window in this window's owning layout list.
    pub(crate) fn set_prev_in_layout(&mut self, prev: Option<&Rc<RefCell<GameWindow>>>) {
        self.prev_in_layout = prev.map(Rc::downgrade);
    }

    /// Get the next window in this window's owning layout list.
    pub(crate) fn get_next_in_layout(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.next_in_layout.as_ref()?.upgrade()
    }

    /// Get the previous window in this window's owning layout list.
    pub(crate) fn get_prev_in_layout(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.prev_in_layout.as_ref()?.upgrade()
    }

    /// Set the next window in this window's sibling list.
    pub(crate) fn set_next_sibling(&mut self, next: Option<&Rc<RefCell<GameWindow>>>) {
        self.next_sibling = next.map(Rc::downgrade);
    }

    /// Set the previous window in this window's sibling list.
    pub(crate) fn set_prev_sibling(&mut self, prev: Option<&Rc<RefCell<GameWindow>>>) {
        self.prev_sibling = prev.map(Rc::downgrade);
    }

    /// Get the next window in this window's sibling list.
    pub(crate) fn get_next_sibling(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.next_sibling.as_ref()?.upgrade()
    }

    /// Get the previous window in this window's sibling list.
    pub(crate) fn get_prev_sibling(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.prev_sibling.as_ref()?.upgrade()
    }

    /// Return the first leaf in this window's root branch.
    pub fn find_first_leaf(window: &Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        let mut leaf = Self::root_of(window);
        loop {
            let child = leaf.borrow().children().first().cloned();
            if let Some(child) = child {
                leaf = child;
            } else {
                return leaf;
            }
        }
    }

    /// Return the last leaf in this window's root branch.
    pub fn find_last_leaf(window: &Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        let mut leaf = Self::root_of(window);
        loop {
            let child = leaf.borrow().children().first().cloned();
            let Some(child) = child else {
                return leaf;
            };
            leaf = Self::last_sibling(child);
        }
    }

    /// Return the previous leaf in C++ tab traversal order.
    pub fn find_prev_leaf(window: &Rc<RefCell<GameWindow>>) -> Option<Rc<RefCell<GameWindow>>> {
        let mut leaf = window.clone();
        if let Some(prev) = leaf.borrow().get_prev_sibling() {
            return Some(Self::last_leaf_from(prev));
        }

        loop {
            let parent = leaf.borrow().get_parent();
            let Some(parent) = parent else {
                return Some(Self::find_last_leaf(&leaf));
            };
            leaf = parent;
            if leaf.borrow().get_parent().is_some() {
                if let Some(prev) = leaf.borrow().get_prev_sibling() {
                    return Some(Self::last_leaf_from(prev));
                }
            }
        }
    }

    /// Return the next leaf in C++ tab traversal order.
    pub fn find_next_leaf(window: &Rc<RefCell<GameWindow>>) -> Option<Rc<RefCell<GameWindow>>> {
        let mut leaf = window.clone();
        if let Some(next) = leaf.borrow().get_next_sibling() {
            return Self::first_leaf_from(next);
        }

        loop {
            let parent = leaf.borrow().get_parent();
            let Some(parent) = parent else {
                return Some(Self::find_first_leaf(&leaf));
            };
            leaf = parent;
            if leaf.borrow().get_parent().is_some() {
                if let Some(next) = leaf.borrow().get_next_sibling() {
                    return Self::first_leaf_from(next);
                }
            }
        }
    }

    pub(crate) fn root_of(window: &Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        let mut root = window.clone();
        loop {
            let parent = root.borrow().get_parent();
            if let Some(parent) = parent {
                root = parent;
            } else {
                return root;
            }
        }
    }

    pub(crate) fn last_sibling(mut window: Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        loop {
            let next = window.borrow().get_next_sibling();
            if let Some(next) = next {
                window = next;
            } else {
                return window;
            }
        }
    }

    pub(crate) fn first_leaf_from(
        mut leaf: Rc<RefCell<GameWindow>>,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        loop {
            let leaf_borrow = leaf.borrow();
            if leaf_borrow.children().is_empty()
                || leaf_borrow.get_status().contains(WindowStatus::TAB_STOP)
            {
                drop(leaf_borrow);
                return Some(leaf);
            }
            let child = leaf_borrow.children().first().cloned().unwrap();
            drop(leaf_borrow);
            leaf = child;
        }
    }

    pub(crate) fn last_leaf_from(mut leaf: Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        loop {
            let descend = {
                let leaf_borrow = leaf.borrow();
                !leaf_borrow.get_status().contains(WindowStatus::TAB_STOP)
                    && !leaf_borrow.children().is_empty()
            };
            if !descend {
                return leaf;
            }
            let child = leaf.borrow().children().first().cloned().unwrap();
            leaf = Self::last_sibling(child);
        }
    }

    /// Add child window
    pub fn add_child(&mut self, child: Rc<RefCell<GameWindow>>) {
        self.children.insert(0, child);
        Self::sync_sibling_links(&self.children);
    }

    /// Remove child window
    pub fn remove_child(&mut self, child: &Rc<RefCell<GameWindow>>) {
        self.children.retain(|c| !Rc::ptr_eq(c, child));
        {
            let mut child = child.borrow_mut();
            child.parent = None;
            child.set_prev_sibling(None);
            child.set_next_sibling(None);
        }
        Self::sync_sibling_links(&self.children);
    }

    /// Get immutable slice of child windows
    pub fn children(&self) -> &[Rc<RefCell<GameWindow>>] {
        &self.children
    }

    /// Get mutable view of the child list
    pub fn children_mut(&mut self) -> &mut Vec<Rc<RefCell<GameWindow>>> {
        &mut self.children
    }

    /// Synchronize C++-style m_next/m_prev links for this window's child list.
    pub(crate) fn sync_child_sibling_links(&mut self) {
        Self::sync_sibling_links(&self.children);
    }

    pub(crate) fn sync_sibling_links(windows: &[Rc<RefCell<GameWindow>>]) {
        for (index, window) in windows.iter().enumerate() {
            let prev = index.checked_sub(1).and_then(|i| windows.get(i));
            let next = windows.get(index + 1);
            let mut window = window.borrow_mut();
            window.set_prev_sibling(prev);
            window.set_next_sibling(next);
        }
    }

    /// Check if a window is a child of this window
    pub fn is_child(&self, window: &GameWindow) -> bool {
        let mut parent = window.get_parent();
        while let Some(parent_rc) = parent {
            let parent_borrow = parent_rc.borrow();
            if std::ptr::eq(self, &*parent_borrow) {
                return true;
            }
            parent = parent_borrow.get_parent();
        }
        false
    }

    /// Check if a window is this window or any descendant.
    pub fn contains_descendant(&self, window: &GameWindow) -> bool {
        if std::ptr::eq(self, window) {
            return true;
        }
        for child_rc in &self.children {
            let child = child_rc.borrow();
            if child.contains_descendant(window) {
                return true;
            }
        }
        false
    }

    /// Get first child window
    pub fn get_first_child(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.children.first().cloned()
    }

    /// Get the decorated name from the instance data.
    pub fn get_name(&self) -> &str {
        &self.inst_data.decorated_name
    }

    /// Set the decorated name for lookup and debugging.
    pub fn set_name(&mut self, name: &str) {
        self.inst_data.decorated_name = name.to_string();
    }

    /// Replace the current status with an explicit value.
    pub fn set_status_exact(&mut self, status: WindowStatus) {
        self.status = status;
        self.inst_data.status = status;
        if let Some(widget) = &mut self.widget {
            widget.set_enabled(status.contains(WindowStatus::ENABLED));
            widget.set_visible(!status.contains(WindowStatus::HIDDEN));
        }
    }

    /// Mutable access to instance data for script loading.
    pub fn instance_data_mut(&mut self) -> &mut WindowInstanceData {
        &mut self.inst_data
    }

    /// Immutable access to instance data for script loading.
    pub fn instance_data(&self) -> &WindowInstanceData {
        &self.inst_data
    }

    pub fn set_video_buffer(&mut self, buffer: Option<VideoBufferHandle>) {
        self.inst_data.video_buffer = buffer;
    }

    pub fn video_buffer(&self) -> Option<VideoBufferHandle> {
        self.inst_data.video_buffer.clone()
    }

    pub(crate) fn ensure_display_text(&mut self) -> Option<DisplayStringHandle> {
        if self.inst_data.display_text.is_none() {
            let handle = {
                let mut manager = get_display_string_manager();
                manager.new_display_string()
            };
            if let Some(font_desc) = self.inst_data.font.as_ref().map(GameFont::to_font_desc) {
                if let Ok(font_ref) = get_font_library().get_font(&font_desc) {
                    handle.borrow_mut().set_font(font_ref);
                }
            }
            self.inst_data.display_text = Some(handle);
        }
        self.inst_data.display_text.clone()
    }

    pub(crate) fn ensure_display_tooltip(&mut self) -> Option<DisplayStringHandle> {
        if self.inst_data.display_tooltip.is_none() {
            let handle = {
                let mut manager = get_display_string_manager();
                manager.new_display_string()
            };
            if let Some(font_desc) = self.inst_data.font.as_ref().map(GameFont::to_font_desc) {
                if let Ok(font_ref) = get_font_library().get_font(&font_desc) {
                    handle.borrow_mut().set_font(font_ref);
                }
            }
            self.inst_data.display_tooltip = Some(handle);
        }
        self.inst_data.display_tooltip.clone()
    }

    /// Attach a gadget widget to this window.
    pub fn set_widget(&mut self, widget: WindowWidget) {
        self.widget = Some(widget);
        self.sync_listbox_content_top_inset();
    }

    pub fn widget(&self) -> Option<&WindowWidget> {
        self.widget.as_ref()
    }

    pub fn widget_mut(&mut self) -> Option<&mut WindowWidget> {
        self.widget.as_mut()
    }

    pub(crate) fn set_combobox_links(&mut self, links: ComboBoxLinks) {
        self.combobox_links = Some(links);
    }

    pub(crate) fn combobox_links(&self) -> Option<ComboBoxLinks> {
        self.combobox_links
    }

    pub(crate) fn set_listbox_links(&mut self, links: ListBoxLinks) {
        self.listbox_links = Some(links);
    }

    pub(crate) fn listbox_links(&self) -> Option<ListBoxLinks> {
        self.listbox_links
    }

    pub(crate) fn set_slider_thumb(&mut self, thumb: WindowId) {
        self.slider_thumb = Some(thumb);
    }

    pub(crate) fn slider_thumb(&self) -> Option<WindowId> {
        self.slider_thumb
    }

    pub fn static_text_mut(&mut self) -> Option<&mut StaticText> {
        match self.widget.as_mut() {
            Some(WindowWidget::StaticText(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn text_entry_mut(&mut self) -> Option<&mut TextEntry> {
        match self.widget.as_mut() {
            Some(WindowWidget::TextEntry(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn list_box_mut(&mut self) -> Option<&mut ListBox> {
        match self.widget.as_mut() {
            Some(WindowWidget::ListBox(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn list_box_selection_result(&self) -> Option<ListBoxSelectionResult> {
        let Some(WindowWidget::ListBox(listbox)) = self.widget.as_ref() else {
            return None;
        };
        let mut result = ListBoxSelectionResult::default();
        match listbox.get_selection() {
            ListBoxSelection::Single(index) => {
                result.single = index;
                result.multiple.clear();
            }
            ListBoxSelection::Multiple(indices) => {
                result.single = -1;
                result.multiple = indices;
            }
        }
        Some(result)
    }

    pub(crate) fn listbox_content_top_inset(&self) -> u32 {
        if self.inst_data.text.is_empty() {
            return 0;
        }
        let font_height = with_window_manager_ref(|manager| {
            self.inst_data
                .font
                .as_ref()
                .map(|font| manager.win_font_height(font))
                .unwrap_or(12)
        });
        (font_height + 1).max(0) as u32
    }

    pub(crate) fn sync_listbox_content_top_inset(&mut self) {
        let inset = self.listbox_content_top_inset();
        let one_line = self.status.contains(WindowStatus::ONE_LINE);
        let (font_height, average_width) = self
            .inst_data
            .font
            .as_ref()
            .map(|font| {
                let height = font.size.max(1) as u32;
                let average_width = ((font.size as f32 * 0.6).round() as i32).max(1) as u32;
                (height, average_width)
            })
            .unwrap_or((18, 8));
        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
            listbox.set_content_top_inset(inset);
            listbox.set_wrap_metrics(one_line, font_height, average_width);
        }
    }

    pub fn combo_box_mut(&mut self) -> Option<&mut ComboBox> {
        match self.widget.as_mut() {
            Some(WindowWidget::ComboBox(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn set_combo_box_selected(&mut self, index: usize, dont_hide: bool) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() else {
            return;
        };
        if dont_hide {
            combo.set_dont_hide_next(true);
        }
        let _ = combo.select_index(index);
        if let Some(links) = self.combobox_links {
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                self.sync_combobox_edit_box(&edit_box);
            }
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                self.sync_combobox_listbox(&list_box);
            }
        }
    }

    pub(crate) fn hide_combobox_list(&mut self) {
        let Some(links) = self.combobox_links else {
            return;
        };
        let Some(list_box) = self.find_child_by_id(links.list_box) else {
            return;
        };
        if list_box.borrow().is_hidden() {
            return;
        }
        let _ = list_box.borrow_mut().hide(true);
        if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
            combo.close();
        }
        if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
            let base_height = edit_box.borrow().get_size().1;
            let (width, _) = self.get_size();
            let _ = self.set_size(width, base_height);
        }
    }

    pub(crate) fn play_gui_click() {
        if let Some(audio) = gamelogic::helpers::TheAudio::get() {
            let event = gamelogic::common::audio::AudioEventRts::new("GUIClick");
            audio.add_audio_event(&event);
        }
    }

    pub(crate) fn claim_combobox_lone_window(&mut self) {
        let window_id = self.id;
        queue_window_manager_op(move |manager| {
            if let Some(window) = manager.get_window_by_id(window_id) {
                manager.set_lone_window(Some(&window));
            }
        });
    }

    pub(crate) fn toggle_combobox_dropdown(&mut self) {
        let Some(links) = self.combobox_links else {
            return;
        };
        let Some(list_box) = self.find_child_by_id(links.list_box) else {
            return;
        };
        if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
            combo.set_dont_hide_next(false);
        }
        self.claim_combobox_lone_window();
        let is_hidden = list_box.borrow().is_hidden();
        if is_hidden {
            self.sync_combobox_listbox(&list_box);
            self.resize_combobox_listbox(&list_box);
            let list_height = list_box.borrow().get_size().1;
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                let base_height = edit_box.borrow().get_size().1;
                let (width, _) = self.get_size();
                let _ = self.set_size(width, base_height + list_height);
            }
            let _ = list_box.borrow_mut().hide(false);
            if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
                combo.open();
            }
        } else {
            self.hide_combobox_list();
        }
    }

    pub(crate) fn set_combobox_editable(&mut self, editable: bool) {
        if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
            combo.set_editable(editable);
        }
        let Some(links) = self.combobox_links else {
            return;
        };
        let Some(edit_box) = self.find_child_by_id(links.edit_box) else {
            return;
        };
        if editable {
            let _ = edit_box.borrow_mut().clear_status(WindowStatus::NO_INPUT);
        } else {
            let _ = edit_box.borrow_mut().set_status(WindowStatus::NO_INPUT);
        }
    }

    pub(crate) fn set_combobox_validation_flags(
        &mut self,
        ascii_only: Option<bool>,
        letters_and_numbers: Option<bool>,
    ) {
        let mut ascii = false;
        let mut alnum = false;
        if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
            if let Some(value) = ascii_only {
                combo.set_ascii_only(value);
            }
            if let Some(value) = letters_and_numbers {
                combo.set_letters_and_numbers(value);
            }
            ascii = combo.ascii_only();
            alnum = combo.letters_and_numbers();
        }
        let mode = if alnum {
            ValidationMode::AlphanumericOnly
        } else if ascii {
            ValidationMode::AsciiOnly
        } else {
            ValidationMode::None
        };
        if let Some(links) = self.combobox_links {
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                if let Some(WindowWidget::TextEntry(entry)) = edit_box.borrow_mut().widget_mut() {
                    entry.set_validation(mode);
                }
            }
        }
    }

    pub(crate) fn handle_combobox_list_selection(
        &mut self,
        links: ComboBoxLinks,
        list_box: &Rc<RefCell<GameWindow>>,
        selected: i32,
    ) -> WindowMsgHandled {
        let dont_hide = if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
            if selected < 0 {
                combo.clear_selection();
            } else {
                let _ = combo.select_index(selected as usize);
            }
            combo.take_dont_hide_next()
        } else {
            false
        };

        if selected >= 0 {
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                let selected_text_and_color =
                    list_box.borrow().widget().and_then(|widget| match widget {
                        WindowWidget::ListBox(listbox) => {
                            Some(listbox.get_text_and_color(selected, 0))
                        }
                        _ => None,
                    });
                if let Some(text_and_color) = selected_text_and_color {
                    if let Some(WindowWidget::TextEntry(entry)) = edit_box.borrow_mut().widget_mut()
                    {
                        entry.set_text_color(Some(gadget_color_from_shell_color(
                            text_and_color.color,
                        )));
                        entry.set_text(text_and_color.text);
                    }
                } else {
                    self.sync_combobox_edit_box(&edit_box);
                }
            }
        }

        if !dont_hide {
            self.hide_combobox_list();
        }

        if selected >= 0 && !self.owner_is_self {
            if let Some(owner) = self.get_owner() {
                let _ = owner.borrow_mut().send_system_message(
                    WindowMessage::User(GCM_SELECTED),
                    self.id as WindowMsgData,
                    0,
                );
            }
        }

        WindowMsgHandled::Handled
    }

    pub fn check_box_mut(&mut self) -> Option<&mut CheckBox> {
        match self.widget.as_mut() {
            Some(WindowWidget::CheckBox(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn gadget_check_box_set_checked(&mut self, checked: bool) -> WindowMsgHandled {
        let Some(WindowWidget::CheckBox(checkbox)) = self.widget.as_mut() else {
            return WindowMsgHandled::Ignored;
        };

        checkbox.set_checked(checked);
        self.sync_state_from_widget();
        self.notify_owner_gadget_selected();
        WindowMsgHandled::Handled
    }

    pub fn gadget_check_box_toggle(&mut self) -> WindowMsgHandled {
        let Some(WindowWidget::CheckBox(checkbox)) = self.widget.as_mut() else {
            return WindowMsgHandled::Ignored;
        };

        checkbox.toggle();
        self.sync_state_from_widget();
        self.notify_owner_gadget_selected();
        WindowMsgHandled::Handled
    }

    pub(crate) fn notify_owner_gadget_selected(&mut self) {
        if self.owner_is_self {
            let _ = self.send_system_message(
                WindowMessage::GadgetSelected,
                self.id as WindowMsgData,
                0,
            );
        } else if let Some(owner) = self.get_owner() {
            let _ = owner.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                self.id as WindowMsgData,
                0,
            );
        }
    }

    pub fn progress_bar_mut(&mut self) -> Option<&mut ProgressBar> {
        match self.widget.as_mut() {
            Some(WindowWidget::ProgressBar(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn horizontal_slider_mut(&mut self) -> Option<&mut HorizontalSlider> {
        match self.widget.as_mut() {
            Some(WindowWidget::HorizontalSlider(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn vertical_slider_mut(&mut self) -> Option<&mut VerticalSlider> {
        match self.widget.as_mut() {
            Some(WindowWidget::VerticalSlider(widget)) => Some(widget),
            _ => None,
        }
    }

    pub(crate) fn set_slider_thumb_hilite_state(&mut self, state: bool) {
        let Some(thumb_id) = self.slider_thumb else {
            return;
        };
        if let Some(thumb) = self.find_child_by_id(thumb_id) {
            if matches!(thumb.borrow().widget(), Some(WindowWidget::PushButton(_))) {
                thumb.borrow_mut().set_hilite_state(state);
            }
        }
    }

    /// Set user data
    pub fn set_user_data<T: 'static>(&mut self, data: T) {
        self.user_data = Some(Box::new(data));
    }

    /// Get user data
    pub fn get_user_data<T: 'static>(&self) -> Option<&T> {
        self.user_data.as_ref()?.downcast_ref::<T>()
    }

    /// Set GUI-editor-only metadata for this window.
    pub fn set_edit_data(&mut self, edit_data: Option<GameWindowEditData>) {
        self.edit_data = edit_data;
    }

    /// Get GUI-editor-only metadata for this window.
    pub fn get_edit_data(&self) -> Option<&GameWindowEditData> {
        self.edit_data.as_ref()
    }

    /// Get mutable GUI-editor-only metadata for this window.
    pub fn get_edit_data_mut(&mut self) -> Option<&mut GameWindowEditData> {
        self.edit_data.as_mut()
    }

    /// Set draw callback
    pub fn set_draw_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, &WindowInstanceData) + 'static,
    {
        self.callbacks.draw = Some(Box::new(callback));
    }

    /// Get draw callback.
    pub fn get_draw_callback(&self) -> Option<&dyn Fn(&GameWindow, &WindowInstanceData)> {
        self.callbacks.draw.as_deref()
    }

    /// Reset draw callback to the legacy default handler.
    pub fn reset_draw_callback(&mut self) {
        self.callbacks.draw = Some(Box::new(legacy_default_draw_callback));
    }

    /// Set input callback
    pub fn set_input_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled
            + 'static,
    {
        self.callbacks.input = Some(Box::new(callback));
    }

    /// Get input callback.
    pub fn get_input_callback(
        &self,
    ) -> Option<&dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>
    {
        self.callbacks.input.as_deref()
    }

    /// Reset input callback to the default handler.
    pub fn reset_input_callback(&mut self) {
        self.callbacks.input = Some(Box::new(default_input_callback));
    }

    /// Set system callback
    pub fn set_system_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled
            + 'static,
    {
        self.callbacks.system = Some(Box::new(callback));
    }

    /// Get system callback.
    pub fn get_system_callback(
        &self,
    ) -> Option<&dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>
    {
        self.callbacks.system.as_deref()
    }

    /// Reset system callback to the default handler.
    pub fn reset_system_callback(&mut self) {
        self.callbacks.system = Some(Box::new(default_system_callback));
    }

    /// Set tooltip callback
    pub fn set_tooltip_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, &WindowInstanceData, u32) + 'static,
    {
        self.callbacks.tooltip = Some(Box::new(callback));
    }

    /// Get tooltip callback.
    pub fn get_tooltip_callback(&self) -> Option<&dyn Fn(&GameWindow, &WindowInstanceData, u32)> {
        self.callbacks.tooltip.as_deref()
    }

    /// Clear tooltip callback.
    pub fn clear_tooltip_callback(&mut self) {
        self.callbacks.tooltip = None;
    }

    /// Set input, draw, and tooltip callbacks in one operation, like C++ winSetCallbacks.
    pub fn set_callbacks(
        &mut self,
        input: Option<InputCallback>,
        draw: Option<DrawCallback>,
        tooltip: Option<TooltipCallback>,
    ) {
        self.callbacks.input = input.or_else(|| Some(Box::new(default_input_callback)));
        self.callbacks.draw = draw.or_else(|| Some(Box::new(legacy_default_draw_callback)));
        self.callbacks.tooltip = tooltip;
    }

    /// Draw the window
    pub fn draw(&self) {
        if !self.is_hidden() {
            if let Some(ref draw_callback) = self.callbacks.draw {
                draw_callback(self, &self.inst_data);
            }
        }
    }

    /// Send input message to window.
    ///
    /// C++ `GameWindowManager::winSendInputMsg` (GameWindowManager.cpp) rejects
    /// only `WIN_STATUS_DESTROYED` (except `GWM_DESTROY`). Hidden, disabled, and
    /// `NO_INPUT` are hit-test filters, not send-time drops — grab/captor/focus
    /// can still deliver input to those windows.
    pub fn send_input_message(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Destroy && self.status.contains(WindowStatus::DESTROYED) {
            return WindowMsgHandled::Ignored;
        }
        self.update_press_state_from_message(msg);
        if let Some(ref input_callback) = self.callbacks.input {
            let result = input_callback(self, msg, data1, data2);
            if result.is_ignored() {
                self.handle_widget_input(msg, data1, data2)
            } else {
                result
            }
        } else {
            self.handle_widget_input(msg, data1, data2)
        }
    }

    /// Send input after legacy window-manager routing has already selected the target.
    pub fn send_routed_input_message(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Destroy && self.status.contains(WindowStatus::DESTROYED) {
            return WindowMsgHandled::Ignored;
        }
        self.update_press_state_from_message(msg);
        if let Some(ref input_callback) = self.callbacks.input {
            let result = input_callback(self, msg, data1, data2);
            if result.is_ignored() {
                self.handle_widget_input(msg, data1, data2)
            } else {
                result
            }
        } else {
            self.handle_widget_input(msg, data1, data2)
        }
    }

    pub(crate) fn update_press_state_from_message(&mut self, msg: WindowMessage) {
        if !self.is_press_anim_enabled() {
            return;
        }
        match msg {
            WindowMessage::LeftDown => {
                if !self.press_was_down {
                    self.press_scale_target = 0.94;
                    self.press_scale_velocity = self.press_impulse;
                    self.press_was_down = true;
                }
            }
            WindowMessage::LeftUp if self.press_was_down => {
                self.press_scale_target = 1.0;
                self.press_scale_velocity = self.release_impulse;
                self.press_was_down = false;
            }
            _ => {}
        }
    }

    /// Send system message to window
    pub fn send_system_message(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Destroy && self.status.contains(WindowStatus::DESTROYED) {
            return WindowMsgHandled::Ignored;
        }

        if let Some(ref system_callback) = self.callbacks.system {
            let result = system_callback(self, msg, data1, data2);
            if result.is_ignored() {
                self.handle_widget_system(msg, data1, data2)
            } else {
                result
            }
        } else {
            self.handle_widget_system(msg, data1, data2)
        }
    }
}

#[cfg(test)]
mod send_input_gate_tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn send_input_message_rejects_only_destroyed_like_cpp() {
        let mut window = GameWindow::new();
        let seen = Rc::new(RefCell::new(0u32));
        {
            let seen = Rc::clone(&seen);
            window.set_input_callback(move |_, _, _, _| {
                *seen.borrow_mut() += 1;
                WindowMsgHandled::Handled
            });
        }

        window.hide(true).unwrap();
        window.enable(false).unwrap();
        let _ = window.set_status(WindowStatus::NO_INPUT);
        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(*seen.borrow(), 1);

        window.set_status_exact(WindowStatus::DESTROYED);
        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(*seen.borrow(), 1);
        assert_eq!(
            window.send_input_message(WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(*seen.borrow(), 2);
    }
}
