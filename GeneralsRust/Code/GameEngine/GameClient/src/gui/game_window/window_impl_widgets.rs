//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use super::prelude::*;

impl GameWindow {
    pub(crate) fn sync_combobox_listbox(&mut self, list_box: &Rc<RefCell<GameWindow>>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_ref() else {
            return;
        };
        let mut list_box_guard = list_box.borrow_mut();
        let Some(WindowWidget::ListBox(listbox)) = list_box_guard.widget_mut() else {
            return;
        };
        listbox.clear();
        for item in combo.items() {
            listbox.add_item(&item.text);
        }
        if let Some(selected) = combo.selected_index() {
            let _ = listbox.select_index(selected, KeyModifiers::none());
        }
        drop(list_box_guard);
        list_box.borrow_mut().update_listbox_scrollbar();
    }

    pub(crate) fn sync_combobox_edit_box(&mut self, edit_box: &Rc<RefCell<GameWindow>>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_ref() else {
            return;
        };
        let mut edit_box_guard = edit_box.borrow_mut();
        let Some(WindowWidget::TextEntry(entry)) = edit_box_guard.widget_mut() else {
            return;
        };
        entry.set_text(combo.text());
    }

    pub(crate) fn combo_box_dropdown_visible_count(
        entry_count: usize,
        max_display: usize,
    ) -> usize {
        if max_display > 0 {
            entry_count.min(max_display)
        } else {
            entry_count
        }
    }

    pub(crate) fn combo_box_dropdown_height(
        entry_count: usize,
        max_display: usize,
        font_height: i32,
    ) -> i32 {
        let visible = Self::combo_box_dropdown_visible_count(entry_count, max_display);
        (font_height.max(0) * visible as i32) + (visible as i32 * 2) + 4
    }

    pub(crate) fn resize_combobox_listbox(&mut self, list_box: &Rc<RefCell<GameWindow>>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_ref() else {
            return;
        };
        let count = combo.items().len();
        let max_display = combo.max_display();
        let visible = Self::combo_box_dropdown_visible_count(count, max_display);
        let show_scrollbar = max_display > 0 && count > max_display;
        let font_height = {
            let list_box_ref = list_box.borrow();
            with_window_manager_ref(|manager| {
                list_box_ref
                    .inst_data
                    .font
                    .as_ref()
                    .map(|font| manager.win_font_height(font))
                    .unwrap_or_else(|| {
                        list_box_ref
                            .widget()
                            .and_then(|widget| match widget {
                                WindowWidget::ListBox(listbox) => {
                                    Some(listbox.item_height().saturating_sub(2) as i32)
                                }
                                _ => None,
                            })
                            .unwrap_or(16)
                    })
            })
        };
        let height = Self::combo_box_dropdown_height(count, max_display, font_height);
        let (width, _) = list_box.borrow().get_size();
        let _ = list_box.borrow_mut().set_size(width, height);
        if let Some(links) = list_box.borrow().listbox_links() {
            if let Some(up) = list_box.borrow().find_child_by_id(links.up_button) {
                let _ = up.borrow_mut().hide(!show_scrollbar);
            }
            if let Some(down) = list_box.borrow().find_child_by_id(links.down_button) {
                let _ = down.borrow_mut().hide(!show_scrollbar);
            }
            if let Some(slider) = list_box.borrow().find_child_by_id(links.slider) {
                let _ = slider.borrow_mut().hide(!show_scrollbar);
            }
        }
        list_box.borrow_mut().update_listbox_scrollbar();
    }

    pub(crate) fn update_listbox_scrollbar(&mut self) {
        let Some(links) = self.listbox_links else {
            return;
        };
        let Some(WindowWidget::ListBox(listbox)) = self.widget.as_ref() else {
            return;
        };

        let bounds = listbox.bounds();
        let item_height = listbox.item_height().max(1) as usize;
        let visible = (bounds.height as usize / item_height).max(1);
        let max_offset = listbox.items().len().saturating_sub(visible);
        let scroll_offset = listbox.scroll_offset().min(max_offset);
        if scroll_offset != listbox.scroll_offset() {
            if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                listbox.set_scroll_offset(scroll_offset);
            }
        }

        if let Some(slider) = self.find_child_by_id(links.slider) {
            if let Some(WindowWidget::VerticalSlider(slider)) = slider.borrow_mut().widget_mut() {
                slider.set_range(0, max_offset as i32);
                slider.set_value(max_offset.saturating_sub(scroll_offset) as i32);
            } else if let Some(WindowWidget::HorizontalSlider(slider)) =
                slider.borrow_mut().widget_mut()
            {
                slider.set_range(0, max_offset as i32);
                slider.set_value(max_offset.saturating_sub(scroll_offset) as i32);
            }
        }

        if let Some(up_button) = self.find_child_by_id(links.up_button) {
            let enabled = max_offset > 0 && scroll_offset > 0;
            let _ = up_button.borrow_mut().enable(enabled);
        }
        if let Some(down_button) = self.find_child_by_id(links.down_button) {
            let enabled = max_offset > 0 && scroll_offset < max_offset;
            let _ = down_button.borrow_mut().enable(enabled);
        }
        if let Some(slider) = self.find_child_by_id(links.slider) {
            let enabled = max_offset > 0;
            let _ = slider.borrow_mut().enable(enabled);
        }

        let mut content_width = bounds.width;
        if let Some(slider) = self.find_child_by_id(links.slider) {
            if !slider.borrow().is_hidden() {
                let (slider_width, _) = slider.borrow().get_size();
                content_width = content_width.saturating_sub(slider_width.max(0) as u32 + 2);
            }
        }
        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
            listbox.set_content_width(content_width);
        }

        if let Some(thumb_id) = links.thumb {
            if let Some(thumb) = self.find_child_by_id(thumb_id) {
                if let Some(slider) = self.find_child_by_id(links.slider) {
                    let (_, slider_height) = slider.borrow().get_size();
                    let (_, thumb_height) = thumb.borrow().get_size();
                    let available = (slider_height - thumb_height).max(0);
                    let ratio = if max_offset > 0 {
                        scroll_offset as f32 / max_offset as f32
                    } else {
                        0.0
                    };
                    let thumb_y = (ratio * available as f32).round() as i32;
                    let _ = thumb.borrow_mut().set_position(0, thumb_y);
                    let _ = thumb.borrow_mut().hide(max_offset == 0);
                }
            }
        }
    }

    pub(crate) fn update_slider_thumb(&mut self) {
        let Some(thumb_id) = self.slider_thumb else {
            return;
        };
        let Some(thumb) = self.find_child_by_id(thumb_id) else {
            return;
        };
        let (thumb_w, thumb_h) = thumb.borrow().get_size();
        let (width, height) = self.get_size();

        match self.widget.as_ref() {
            Some(WindowWidget::HorizontalSlider(slider)) => {
                let (min_val, max_val) = slider.range();
                let range = (max_val - min_val).max(1);
                let track = (width - thumb_w).max(0);
                let ratio = (slider.value() - min_val) as f32 / range as f32;
                let x = (ratio * track as f32).round() as i32;
                let _ = thumb
                    .borrow_mut()
                    .set_position(x, HORIZONTAL_SLIDER_THUMB_POSITION);
            }
            Some(WindowWidget::VerticalSlider(slider)) => {
                let (min_val, max_val) = slider.range();
                let range = (max_val - min_val).max(1);
                let track = (height - thumb_h).max(0);
                let ratio = (max_val - slider.value()) as f32 / range as f32;
                let y = (ratio * track as f32).round() as i32;
                let _ = thumb.borrow_mut().set_position(0, y);
            }
            _ => {}
        }
    }

    pub(crate) fn slider_value(&self) -> Option<i32> {
        match self.widget.as_ref() {
            Some(WindowWidget::HorizontalSlider(slider)) => Some(slider.value()),
            Some(WindowWidget::VerticalSlider(slider)) => Some(slider.value()),
            _ => None,
        }
    }

    pub(crate) fn apply_slider_value(&mut self, value: i32) {
        match self.widget.as_mut() {
            Some(WindowWidget::HorizontalSlider(slider)) => slider.set_value(value),
            Some(WindowWidget::VerticalSlider(slider)) => slider.set_value(value),
            _ => {}
        }
        self.update_slider_thumb();
    }

    pub(crate) fn notify_slider_track(&mut self) {
        let Some(value) = self.slider_value() else {
            return;
        };
        if self.owner_is_self {
            return;
        }
        if let Some(owner) = self.get_owner() {
            let _ = owner.borrow_mut().send_system_message(
                WindowMessage::User(GSM_SLIDER_TRACK),
                self.id as WindowMsgData,
                value as WindowMsgData,
            );
        }
    }

    pub(crate) fn handle_slider_left_drag(
        &mut self,
        packed_mouse: WindowMsgData,
    ) -> WindowMsgHandled {
        let mouse_x = (packed_mouse & 0xFFFF) as i32;
        let mouse_y = ((packed_mouse >> 16) & 0xFFFF) as i32;
        let (win_x, win_y) = self.get_screen_position();
        let (width, height) = self.get_size();
        let is_horizontal = matches!(self.widget, Some(WindowWidget::HorizontalSlider(_)));
        let Some(thumb_id) = self.slider_thumb else {
            return WindowMsgHandled::Handled;
        };
        let Some(thumb) = self.find_child_by_id(thumb_id) else {
            return WindowMsgHandled::Handled;
        };
        let (thumb_w, thumb_h) = thumb.borrow().get_size();
        let (thumb_sx, thumb_sy) = thumb.borrow().get_screen_position();
        let (thumb_rel_x, _thumb_rel_y) = thumb.borrow().get_position();
        let child_center_x = thumb_sx + thumb_w / 2;
        let child_center_y = thumb_sy + thumb_h / 2;
        let (min_val, max_val) = match self.widget.as_ref() {
            Some(WindowWidget::HorizontalSlider(slider)) => slider.range(),
            Some(WindowWidget::VerticalSlider(slider)) => slider.range(),
            _ => return WindowMsgHandled::Handled,
        };
        let span = (max_val - min_val).max(1);

        if is_horizontal {
            if mouse_x > win_x + width - HORIZONTAL_SLIDER_THUMB_WIDTH / 2 {
                self.apply_slider_value(max_val);
                self.notify_slider_track();
                return WindowMsgHandled::Handled;
            }
            if mouse_x < win_x + HORIZONTAL_SLIDER_THUMB_WIDTH / 2 {
                self.apply_slider_value(min_val);
                self.notify_slider_track();
                return WindowMsgHandled::Handled;
            }
            if child_center_x < win_x + thumb_w / 2 {
                let _ = thumb
                    .borrow_mut()
                    .set_position(0, HORIZONTAL_SLIDER_THUMB_POSITION);
                self.apply_slider_value(min_val);
            } else if child_center_x >= win_x + width - thumb_w / 2 {
                let track = (width - HORIZONTAL_SLIDER_THUMB_WIDTH).max(0);
                let _ = thumb.borrow_mut().set_position(
                    track - HORIZONTAL_SLIDER_THUMB_WIDTH / 2,
                    HORIZONTAL_SLIDER_THUMB_POSITION,
                );
                self.apply_slider_value(max_val);
            } else {
                let num_ticks = (width - HORIZONTAL_SLIDER_THUMB_WIDTH) as f32 / span as f32;
                let delta = child_center_x - win_x - HORIZONTAL_SLIDER_THUMB_WIDTH / 2;
                let mut position = if num_ticks.abs() > f32::EPSILON {
                    (delta as f32 / num_ticks) as i32 + min_val
                } else {
                    min_val
                };
                position = position.clamp(min_val, max_val);
                match self.widget.as_mut() {
                    Some(WindowWidget::HorizontalSlider(slider)) => slider.set_value(position),
                    Some(WindowWidget::VerticalSlider(slider)) => slider.set_value(position),
                    _ => {}
                }
                let _ = thumb
                    .borrow_mut()
                    .set_position(thumb_rel_x, HORIZONTAL_SLIDER_THUMB_POSITION);
            }
        } else if mouse_y > win_y + height {
            self.apply_slider_value(min_val);
            self.notify_slider_track();
            return WindowMsgHandled::Handled;
        } else if mouse_y < win_y {
            self.apply_slider_value(max_val);
            self.notify_slider_track();
            return WindowMsgHandled::Handled;
        } else if child_center_y <= win_y + thumb_h / 2 {
            let _ = thumb.borrow_mut().set_position(0, 0);
            self.apply_slider_value(max_val);
        } else if child_center_y >= win_y + height - thumb_h / 2 {
            let _ = thumb
                .borrow_mut()
                .set_position(0, (height - thumb_h).max(0));
            self.apply_slider_value(min_val);
        } else {
            let num_ticks = (height - thumb_h) as f32 / span as f32;
            let delta = child_center_y - win_y - thumb_h / 2;
            let mut position = if num_ticks.abs() > f32::EPSILON {
                (delta as f32 / num_ticks) as i32
            } else {
                0
            };
            if position > max_val {
                position = max_val;
            }
            position = max_val - position;
            match self.widget.as_mut() {
                Some(WindowWidget::HorizontalSlider(slider)) => slider.set_value(position),
                Some(WindowWidget::VerticalSlider(slider)) => slider.set_value(position),
                _ => {}
            }
        }

        self.notify_slider_track();
        WindowMsgHandled::Handled
    }

    pub(crate) fn show_tab_pane(&mut self, index: usize) {
        let panes: Vec<Rc<RefCell<GameWindow>>> = self
            .children
            .iter()
            .rev()
            .filter(|child| {
                let child = child.borrow();
                (child.inst_data.style & GWS_TAB_PANE) != 0
            })
            .cloned()
            .collect();

        if panes.is_empty() {
            return;
        }

        let mut active_index = if panes.get(index).is_some() { index } else { 0 };
        if let Some(WindowWidget::TabControl(tab_control)) = &mut self.widget {
            let tab_count = tab_control.tab_count();
            if tab_count > 0 {
                active_index = active_index.min(tab_count - 1);
            }
            active_index = active_index.min(panes.len() - 1);
            tab_control.set_active_tab_index_silent(active_index);
        }

        for pane in panes.iter() {
            let _ = pane.borrow_mut().hide(true);
        }

        if let Some(pane) = panes.get(active_index) {
            let _ = pane.borrow_mut().hide(false);
        }
    }

    pub(crate) fn resize_tab_panes_to_content(&mut self) {
        let Some(WindowWidget::TabControl(tab_control)) = self.widget.as_ref() else {
            return;
        };

        let (win_width, win_height) = self.get_size();
        let mut width = win_width - (2 * tab_control.pane_border());
        let mut height = win_height - (2 * tab_control.pane_border());

        if tab_control.tab_edge() == crate::gui::gadgets::tabcontrol::TP_TOP_SIDE
            || tab_control.tab_edge() == crate::gui::gadgets::tabcontrol::TP_BOTTOM_SIDE
        {
            height -= tab_control.tab_height_px();
        }
        if tab_control.tab_edge() == crate::gui::gadgets::tabcontrol::TP_LEFT_SIDE
            || tab_control.tab_edge() == crate::gui::gadgets::tabcontrol::TP_RIGHT_SIDE
        {
            width -= tab_control.tab_width_px();
        }

        let mut x = tab_control.pane_border();
        let mut y = tab_control.pane_border();
        if tab_control.tab_edge() == crate::gui::gadgets::tabcontrol::TP_LEFT_SIDE {
            x += tab_control.tab_width_px();
        }
        if tab_control.tab_edge() == crate::gui::gadgets::tabcontrol::TP_TOP_SIDE {
            y += tab_control.tab_height_px();
        }

        let panes: Vec<Rc<RefCell<GameWindow>>> = self
            .children
            .iter()
            .rev()
            .filter(|child| {
                let child = child.borrow();
                (child.inst_data.style & GWS_TAB_PANE) != 0
            })
            .cloned()
            .collect();

        for pane in panes {
            let mut pane = pane.borrow_mut();
            let _ = pane.set_size(width.max(0), height.max(0));
            let _ = pane.set_position(x, y);
        }
    }

    /// Normalize window region (ensure low < high)
    pub(crate) fn normalize_region(&mut self) {
        if self.region.low.x > self.region.high.x {
            std::mem::swap(&mut self.region.low.x, &mut self.region.high.x);
        }
        if self.region.low.y > self.region.high.y {
            std::mem::swap(&mut self.region.low.y, &mut self.region.high.y);
        }
    }
}
