//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use super::prelude::*;

impl GameWindow {
    pub(crate) fn handle_widget_input(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        self.sync_listbox_content_top_inset();
        if matches!(self.widget, Some(WindowWidget::ComboBox(_)))
            && msg == WindowMessage::LeftUp
            && self.combobox_links.is_some()
        {
            Self::play_gui_click();
            self.toggle_combobox_dropdown();
            return WindowMsgHandled::Handled;
        }

        let Some(widget) = self.widget.as_mut() else {
            return WindowMsgHandled::Ignored;
        };

        if matches!(widget, WindowWidget::ListBox(_))
            && (msg == WindowMessage::WheelUp || msg == WindowMessage::WheelDown)
        {
            let delta = if msg == WindowMessage::WheelUp { -1 } else { 1 };
            if let WindowWidget::ListBox(listbox) = widget {
                listbox.scroll_by(delta);
            }
            self.update_listbox_scrollbar();
            return WindowMsgHandled::Handled;
        }

        let cursor = self.cursor_pos.get();
        let (x, y) = (cursor.x, cursor.y);
        let event = match msg {
            WindowMessage::MousePos => Some(InputEvent::MouseMove { x, y }),
            WindowMessage::MouseEntering => Some(InputEvent::MouseEnter { x, y }),
            WindowMessage::MouseLeaving => Some(InputEvent::MouseLeave { x, y }),
            WindowMessage::LeftDown => Some(InputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            }),
            WindowMessage::LeftUp => Some(InputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            }),
            WindowMessage::LeftDrag => Some(InputEvent::MouseDrag {
                x,
                y,
                button: MouseButton::Left,
            }),
            WindowMessage::MiddleDown => Some(InputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Middle,
            }),
            WindowMessage::MiddleUp => Some(InputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Middle,
            }),
            WindowMessage::MiddleDrag => Some(InputEvent::MouseDrag {
                x,
                y,
                button: MouseButton::Middle,
            }),
            WindowMessage::RightDown => Some(InputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
            }),
            WindowMessage::RightUp => Some(InputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Right,
            }),
            WindowMessage::RightDrag => Some(InputEvent::MouseDrag {
                x,
                y,
                button: MouseButton::Right,
            }),
            WindowMessage::Char => char_input_event(data1, data2),
            WindowMessage::ImeChar => None,
            _ => None,
        };

        if msg == WindowMessage::ImeChar {
            if let WindowWidget::TextEntry(entry) = widget {
                if let Some(ch) = char::from_u32((data1 as u32) & 0xffff) {
                    entry.apply_ime_char(ch);
                    if ch == '\r' || ch == '\n' {
                        if !self.owner_is_self {
                            if let Some(owner) = self.get_owner() {
                                let _ = owner.borrow_mut().send_system_message(
                                    WindowMessage::User(GEM_EDIT_DONE),
                                    self.id as WindowMsgData,
                                    0,
                                );
                            }
                        }
                    } else if ch != '\0' {
                        if !self.owner_is_self {
                            if let Some(owner) = self.get_owner() {
                                let _ = owner.borrow_mut().send_system_message(
                                    WindowMessage::User(GEM_UPDATE_TEXT),
                                    self.id as WindowMsgData,
                                    0,
                                );
                            }
                        }
                    }
                }
                return WindowMsgHandled::Handled;
            }
            return WindowMsgHandled::Ignored;
        }

        if matches!(widget, WindowWidget::TextEntry(_))
            && crate::gui::ime_manager::ime_should_swallow_input_for_window(self.id)
        {
            return WindowMsgHandled::Handled;
        }

        let Some(event) = event else {
            return WindowMsgHandled::Ignored;
        };

        if matches!(
            event,
            InputEvent::MouseEnter { .. } | InputEvent::MouseLeave { .. }
        ) && (self.inst_data.style & GWS_MOUSE_TRACK == 0)
            && !matches!(
                widget,
                WindowWidget::HorizontalSlider(_) | WindowWidget::VerticalSlider(_)
            )
        {
            return WindowMsgHandled::Ignored;
        }

        let state_before = widget.state();
        let messages = widget.handle_input(&event);
        let state_changed = widget.state() != state_before;
        self.sync_state_from_widget();
        if messages.is_empty() {
            return if state_changed {
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            };
        }

        let mut slider_thumb_hilite_handled = false;
        if matches!(
            self.widget,
            Some(WindowWidget::HorizontalSlider(_)) | Some(WindowWidget::VerticalSlider(_))
        ) {
            if matches!(event, InputEvent::MouseEnter { .. }) {
                self.set_slider_thumb_hilite_state(true);
                slider_thumb_hilite_handled = true;
            } else if matches!(event, InputEvent::MouseLeave { .. }) {
                self.set_slider_thumb_hilite_state(false);
                slider_thumb_hilite_handled = true;
            }
            self.update_slider_thumb();
        }

        if matches!(self.widget, Some(WindowWidget::ListBox(_))) {
            self.update_listbox_scrollbar();
        }

        if matches!(self.widget, Some(WindowWidget::TabControl(_))) {
            if let Some(selected) = messages.iter().find_map(|message| {
                if let GadgetMessage::ValueChanged { value, .. } = message {
                    if let GadgetValue::Integer(val) = value {
                        return Some(*val);
                    }
                }
                None
            }) {
                if selected >= 0 {
                    self.show_tab_pane(selected as usize);
                }
            }
        }

        let mut handled = slider_thumb_hilite_handled;
        let gadget_consumed_input = !messages.is_empty();
        let is_checkbox_message = matches!(self.widget, Some(WindowWidget::CheckBox(_)));
        let is_radio_message = matches!(self.widget, Some(WindowWidget::RadioButton(_)));
        let target_owner = if !self.owner_is_self
            && (self.get_parent().is_some() || is_checkbox_message || is_radio_message)
        {
            self.get_owner()
        } else {
            None
        };
        let is_listbox_message = matches!(self.widget, Some(WindowWidget::ListBox(_)));
        let is_slider_message = matches!(
            self.widget,
            Some(WindowWidget::HorizontalSlider(_)) | Some(WindowWidget::VerticalSlider(_))
        );

        let original_data1 = data1;
        for message in messages {
            let (msg, data1, data2) = match message {
                GadgetMessage::Clicked { .. } if is_listbox_message => {
                    continue;
                }
                GadgetMessage::Clicked { .. } if is_checkbox_message || is_radio_message => (
                    WindowMessage::GadgetSelected,
                    self.id as WindowMsgData,
                    original_data1,
                ),
                GadgetMessage::Clicked { .. } => {
                    (WindowMessage::GadgetSelected, self.id as WindowMsgData, 0)
                }
                GadgetMessage::RightClicked { .. } if is_listbox_message => {
                    let (sx, sy) = self.get_screen_position();
                    let payload = match &self.widget {
                        Some(WindowWidget::ListBox(listbox)) => listbox
                            .last_right_click()
                            .map(|right_click| RightClickStruct {
                                pos: right_click.index,
                                mouse_x: right_click.mouse_x + sx,
                                mouse_y: right_click.mouse_y + sy,
                            })
                            .unwrap_or(RightClickStruct {
                                pos: -1,
                                mouse_x: sx,
                                mouse_y: sy,
                            }),
                        _ => RightClickStruct {
                            pos: -1,
                            mouse_x: sx,
                            mouse_y: sy,
                        },
                    };
                    (
                        WindowMessage::User(GLM_RIGHT_CLICKED),
                        self.id as WindowMsgData,
                        push_payload(WindowMsgPayload::RightClick(payload)),
                    )
                }

                GadgetMessage::RightClicked { .. } => {
                    // C++ GadgetCheckBox GWM_RIGHT_UP always sends GBM_SELECTED_RIGHT
                    // when the box was right-pressed. Push buttons still require
                    // WIN_STATUS_RIGHT_CLICK.
                    if !is_checkbox_message && !self.status.contains(WindowStatus::RIGHT_CLICK) {
                        continue;
                    }
                    (
                        WindowMessage::GadgetRightClick,
                        self.id as WindowMsgData,
                        if is_checkbox_message {
                            original_data1
                        } else {
                            0
                        },
                    )
                }
                GadgetMessage::LeftDrag { .. } if is_checkbox_message || is_radio_message => (
                    WindowMessage::User(GGM_LEFT_DRAG),
                    self.id as WindowMsgData,
                    original_data1,
                ),
                GadgetMessage::LeftDrag { .. } => (
                    WindowMessage::User(GGM_LEFT_DRAG),
                    self.id as WindowMsgData,
                    original_data1,
                ),

                GadgetMessage::ValueChanged { value, .. } if is_listbox_message => {
                    let selected = match value {
                        GadgetValue::Integer(row) => row,
                        _ => match &self.widget {
                            Some(WindowWidget::ListBox(listbox)) => listbox
                                .selected_indices()
                                .first()
                                .map(|index| *index as i32)
                                .unwrap_or(-1),
                            _ => -1,
                        },
                    };
                    (
                        WindowMessage::User(GLM_SELECTED),
                        self.id as WindowMsgData,
                        selected as WindowMsgData,
                    )
                }
                GadgetMessage::ValueChanged { value, .. } if is_slider_message => {
                    let position = match value {
                        GadgetValue::Integer(position) => position,
                        _ => self.slider_value().unwrap_or(0),
                    };
                    (
                        WindowMessage::User(GSM_SLIDER_TRACK),
                        self.id as WindowMsgData,
                        position as WindowMsgData,
                    )
                }
                GadgetMessage::ValueChanged { .. } => (
                    WindowMessage::GadgetValueChanged,
                    self.id as WindowMsgData,
                    0,
                ),

                GadgetMessage::EditingComplete { .. } => {
                    (WindowMessage::GadgetEditDone, self.id as WindowMsgData, 0)
                }
                GadgetMessage::MouseEnter { .. } if is_checkbox_message || is_radio_message => (
                    WindowMessage::GadgetMouseEntering,
                    self.id as WindowMsgData,
                    original_data1,
                ),
                GadgetMessage::MouseEnter { .. } => (
                    WindowMessage::GadgetMouseEntering,
                    self.id as WindowMsgData,
                    0,
                ),
                GadgetMessage::MouseLeave { .. } if is_checkbox_message || is_radio_message => (
                    WindowMessage::GadgetMouseLeaving,
                    self.id as WindowMsgData,
                    original_data1,
                ),
                GadgetMessage::MouseLeave { .. } => (
                    WindowMessage::GadgetMouseLeaving,
                    self.id as WindowMsgData,
                    0,
                ),
                GadgetMessage::FocusChanged { has_focus, .. } => {
                    (WindowMessage::InputFocus, if has_focus { 1 } else { 0 }, 0)
                }
                GadgetMessage::Custom { data, .. } => {
                    if data == "tab_next" {
                        with_window_manager(|manager| manager.navigate_tab(TabDirection::Next));
                        handled = true;
                        continue;
                    }
                    if data == "tab_prev" {
                        with_window_manager(|manager| manager.navigate_tab(TabDirection::Previous));
                        handled = true;
                        continue;
                    }
                    if data == "input_handled" {
                        handled = true;
                        continue;
                    }
                    if data == "lone_window" {
                        // C++ GadgetComboBox.cpp:134 / 618 winSetLoneWindow
                        self.claim_combobox_lone_window();
                        handled = true;
                        continue;
                    }
                    if is_listbox_message && data == "double_click" {
                        let selected = match &self.widget {
                            Some(WindowWidget::ListBox(listbox)) => listbox
                                .last_double_click_index()
                                .or_else(|| listbox.selected_indices().first().copied())
                                .map(|index| index as i32)
                                .unwrap_or(-1),
                            _ => -1,
                        };
                        (
                            WindowMessage::User(GLM_DOUBLE_CLICKED),
                            self.id as WindowMsgData,
                            selected as WindowMsgData,
                        )
                    } else {
                        (WindowMessage::User(0x8000), self.id as WindowMsgData, 0)
                    }
                }
            };

            let result = if let Some(ref owner) = target_owner {
                owner.borrow_mut().send_system_message(msg, data1, data2)
            } else {
                self.send_system_message(msg, data1, data2)
            };
            if result.is_handled() {
                handled = true;
            }
        }

        // C++ gadget input returns MSG_HANDLED when the widget consumed the
        // click even if the owner system callback ignores GadgetSelected.
        if gadget_consumed_input {
            handled = true;
        }

        if handled {
            WindowMsgHandled::Handled
        } else {
            WindowMsgHandled::Ignored
        }
    }

    pub(crate) fn unselect_radio_peers_by_group(&mut self, group_id: u32) {
        let Some(parent) = self.get_parent() else {
            return;
        };
        let root = Self::root_of(&parent);
        Self::unselect_radio_peers_in_subtree(&root, group_id, self.id);
    }

    pub(crate) fn unselect_radio_peers_in_subtree(
        window: &Rc<RefCell<GameWindow>>,
        group_id: u32,
        except_id: WindowId,
    ) {
        let children = {
            let Ok(mut window) = window.try_borrow_mut() else {
                return;
            };
            if window.id != except_id {
                let should_clear = match window.widget.as_mut() {
                    Some(WindowWidget::RadioButton(radio)) if radio.group_id() == group_id => {
                        radio.deselect_for_group_update();
                        true
                    }
                    _ => false,
                };
                if should_clear {
                    window.inst_data.state.remove(WindowState::SELECTED);
                }
            }
            window.children.clone()
        };

        for child in children {
            Self::unselect_radio_peers_in_subtree(&child, group_id, except_id);
        }
    }

    pub(crate) fn handle_widget_system(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if matches!(
            msg,
            WindowMessage::RightDown | WindowMessage::RightUp | WindowMessage::RightDrag
        ) && !self.status.contains(WindowStatus::RIGHT_CLICK)
            && !matches!(self.widget, Some(WindowWidget::CheckBox(_)))
        {
            return WindowMsgHandled::Ignored;
        }

        if matches!(
            msg,
            WindowMessage::MouseEntering | WindowMessage::MouseLeaving
        ) && (self.inst_data.style & GWS_MOUSE_TRACK == 0)
        {
            return WindowMsgHandled::Ignored;
        }

        if self.widget.is_none() {
            return WindowMsgHandled::Ignored;
        }

        if let WindowMessage::User(code) = msg {
            match code {
                GGM_SET_LABEL | GEM_SET_TEXT => {
                    if let Some(text) = payload_text(data1) {
                        let _ = self.set_text(&text);
                    }
                    return WindowMsgHandled::Handled;
                }
                GGM_GET_LABEL | GEM_GET_TEXT => {
                    let _ =
                        replace_payload(data2, WindowMsgPayload::Text(self.get_text().to_string()));
                    return WindowMsgHandled::Handled;
                }
                _ => {}
            }
        }

        if matches!(self.widget, Some(WindowWidget::ComboBox(_))) {
            if msg == WindowMessage::Destroy {
                queue_window_manager_op(|manager| manager.set_lone_window(None));
                return WindowMsgHandled::Handled;
            }

            if let WindowMessage::User(code) = msg {
                match code {
                    GCM_GET_TEXT => {
                        let text = gadget_combo_box_get_text(self);
                        let _ = replace_payload(data2, WindowMsgPayload::Text(text));
                        return WindowMsgHandled::Handled;
                    }
                    GCM_SET_TEXT => {
                        if let Some(text) = payload_text(data1) {
                            let mut set_child = false;
                            if let Some(links) = self.combobox_links {
                                if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                    if let Some(WindowWidget::TextEntry(entry)) =
                                        edit_box.borrow_mut().widget_mut()
                                    {
                                        entry.set_text(text.clone());
                                        set_child = true;
                                    }
                                }
                            }
                            if !set_child {
                                if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
                                    combo.set_text(text);
                                }
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GCM_ADD_ENTRY => {
                        let added_index = if let Some(text) = payload_text(data1) {
                            gadget_combo_box_add_entry(
                                self,
                                &text,
                                shell_color_from_packed_arg(data2),
                            )
                        } else {
                            -1
                        };
                        return WindowMsgHandled::Value(added_index as WindowMsgData);
                    }
                    GCM_DEL_ALL => {
                        if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
                            combo.clear();
                        }
                        if let Some(links) = self.combobox_links {
                            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                                if let Some(WindowWidget::ListBox(listbox)) =
                                    list_box.borrow_mut().widget_mut()
                                {
                                    listbox.clear();
                                }
                                list_box.borrow_mut().update_listbox_scrollbar();
                            }
                            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                if let Some(WindowWidget::TextEntry(entry)) =
                                    edit_box.borrow_mut().widget_mut()
                                {
                                    entry.set_text(String::new());
                                }
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GCM_DEL_ENTRY => {
                        return WindowMsgHandled::Handled;
                    }
                    GGM_CLOSE => {
                        self.hide_combobox_list();
                        return WindowMsgHandled::Handled;
                    }
                    GCM_SET_SELECTION => {
                        let selected = data1 as i32;
                        if let Some(links) = self.combobox_links {
                            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                                if !list_box.borrow().is_hidden() && data2 != 0 {
                                    if let Some(WindowWidget::ComboBox(combo)) =
                                        self.widget.as_mut()
                                    {
                                        combo.set_dont_hide_next(true);
                                    }
                                }
                                if let Some(WindowWidget::ListBox(listbox)) =
                                    list_box.borrow_mut().widget_mut()
                                {
                                    if selected < 0 {
                                        listbox.set_selected_indices(&[]);
                                    } else {
                                        let _ = listbox
                                            .select_index(selected as usize, KeyModifiers::none());
                                    }
                                }
                                list_box.borrow_mut().update_listbox_scrollbar();
                                return self
                                    .handle_combobox_list_selection(links, &list_box, selected);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GCM_GET_SELECTION => {
                        let selected = if let Some(links) = self.combobox_links {
                            self.find_child_by_id(links.list_box)
                                .and_then(|list_box| {
                                    list_box.borrow().widget().and_then(|widget| match widget {
                                        WindowWidget::ListBox(listbox) => listbox
                                            .selected_indices()
                                            .first()
                                            .copied()
                                            .map(|index| index as i32),
                                        _ => None,
                                    })
                                })
                                .unwrap_or(-1)
                        } else {
                            -1
                        };
                        let _ = replace_payload(data2, WindowMsgPayload::Int(selected));
                        return WindowMsgHandled::Handled;
                    }
                    GCM_SET_ITEM_DATA => {
                        if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
                            let index = data1 as i32;
                            if index >= 0 {
                                let _ = combo.set_item_data_raw(index as usize, data2);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GCM_GET_ITEM_DATA => {
                        let data = if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_ref()
                        {
                            let index = data1 as i32;
                            if index >= 0 {
                                combo.item_data_raw(index as usize).unwrap_or(0)
                            } else {
                                0
                            }
                        } else {
                            0
                        };
                        let _ = replace_payload(data2, WindowMsgPayload::UInt(data));
                        return WindowMsgHandled::Handled;
                    }
                    _ => {}
                }
            }

            if let Some(links) = self.combobox_links {
                if msg == WindowMessage::GadgetSelected && data1 == links.drop_down as WindowMsgData
                {
                    Self::play_gui_click();
                    self.toggle_combobox_dropdown();
                    return WindowMsgHandled::Handled;
                }

                if msg == WindowMessage::GadgetValueChanged
                    && data1 == links.list_box as WindowMsgData
                {
                    if let Some(list_box) = self.find_child_by_id(links.list_box) {
                        let selected = list_box
                            .borrow()
                            .widget()
                            .and_then(|widget| match widget {
                                WindowWidget::ListBox(listbox) => {
                                    listbox.selected_indices().first().copied()
                                }
                                _ => None,
                            })
                            .map(|index| index as i32)
                            .unwrap_or(-1);
                        return self.handle_combobox_list_selection(links, &list_box, selected);
                    }
                }

                if msg == WindowMessage::User(GLM_SELECTED)
                    && data1 == links.list_box as WindowMsgData
                {
                    if let Some(list_box) = self.find_child_by_id(links.list_box) {
                        return self.handle_combobox_list_selection(links, &list_box, data2 as i32);
                    }
                }

                if msg == WindowMessage::GadgetValueChanged
                    && data1 == links.edit_box as WindowMsgData
                {
                    if !self.owner_is_self {
                        if let Some(owner) = self.get_owner() {
                            let _ = owner.borrow_mut().send_system_message(
                                WindowMessage::User(GCM_UPDATE_TEXT),
                                self.id as WindowMsgData,
                                0,
                            );
                        }
                    }
                    if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
                        combo.clear_selection();
                    }
                    if let Some(list_box) = self.find_child_by_id(links.list_box) {
                        if let Some(WindowWidget::ListBox(listbox)) =
                            list_box.borrow_mut().widget_mut()
                        {
                            listbox.set_selected_indices(&[]);
                        }
                    }
                    self.hide_combobox_list();
                    return WindowMsgHandled::Handled;
                }

                if msg == WindowMessage::GadgetEditDone && data1 == links.edit_box as WindowMsgData
                {
                    if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                        let edit_text =
                            edit_box.borrow().widget().and_then(|widget| match widget {
                                WindowWidget::TextEntry(entry) => {
                                    Some(entry.displayed_text().to_string())
                                }
                                _ => None,
                            });
                        if let (Some(text), Some(WindowWidget::ComboBox(combo))) =
                            (edit_text, self.widget.as_mut())
                        {
                            combo.set_text(&text);
                        }
                        self.hide_combobox_list();
                        if !self.owner_is_self {
                            if let Some(owner) = self.get_owner() {
                                let _ = owner.borrow_mut().send_system_message(
                                    WindowMessage::User(GCM_SELECTED),
                                    self.id as WindowMsgData,
                                    0,
                                );
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                }
            }
        }

        if matches!(self.widget, Some(WindowWidget::ListBox(_))) {
            if let WindowMessage::User(code) = msg {
                match code {
                    GLM_ADD_ENTRY => {
                        let mut added_index = -1;
                        if let Some(WindowMsgPayload::AddEntry(entry)) = payload(data1) {
                            if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                                added_index = listbox.add_entry(entry);
                            }
                        }
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Value(added_index as WindowMsgData);
                    }
                    GLM_DEL_ALL => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            listbox.clear();
                        }
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Handled;
                    }
                    GLM_DEL_ENTRY => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            let _ = listbox.remove_item(data1);
                        }
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Handled;
                    }
                    GLM_SET_SELECTION => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            if listbox.selection_mode() == SelectionMode::Multiple {
                                if let Some(WindowMsgPayload::IntList(select_list)) = payload(data1)
                                {
                                    let indices = select_list
                                        .iter()
                                        .take_while(|&&index| index >= 0)
                                        .filter_map(|&index| {
                                            (index as usize)
                                                .lt(&listbox.items().len())
                                                .then_some(index as usize)
                                        })
                                        .collect::<Vec<_>>();
                                    listbox.set_selected_indices(&indices);
                                } else {
                                    let select_index = data1 as i32;
                                    if select_index < 0
                                        || select_index as usize >= listbox.items().len()
                                    {
                                        listbox.set_selected_indices(&[]);
                                    } else {
                                        let _ = listbox.select_index(
                                            select_index as usize,
                                            KeyModifiers::none(),
                                        );
                                    }
                                }
                            } else {
                                let select_index = data1 as i32;
                                if select_index < 0
                                    || select_index as usize >= listbox.items().len()
                                {
                                    listbox.set_selected_indices(&[]);
                                } else {
                                    let _ = listbox
                                        .select_index(select_index as usize, KeyModifiers::none());
                                }
                            }
                        }
                        self.update_listbox_scrollbar();
                        if !self.owner_is_self {
                            if let Some(owner) = self.get_owner() {
                                let selected = self
                                    .list_box_mut()
                                    .and_then(|listbox| listbox.selected_indices().first().copied())
                                    .map(|index| index as i32)
                                    .unwrap_or(-1);
                                let _ = owner.borrow_mut().send_system_message(
                                    WindowMessage::User(GLM_SELECTED),
                                    self.id as WindowMsgData,
                                    selected as WindowMsgData,
                                );
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GLM_GET_SELECTION => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_ref() {
                            let selection = match listbox.get_selection() {
                                ListBoxSelection::Single(index) => WindowMsgPayload::Int(index),
                                ListBoxSelection::Multiple(indices) => {
                                    WindowMsgPayload::IntList(indices)
                                }
                            };
                            let _ = replace_payload(data2, selection);
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GLM_TOGGLE_MULTI_SELECTION => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            let _ = listbox.toggle_multi_selection(data1 as i32);
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GLM_GET_TEXT => {
                        if let Some(WindowMsgPayload::CellPosition(pos)) = payload(data1) {
                            if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                                let _ = replace_payload(
                                    data2,
                                    WindowMsgPayload::TextAndColor(
                                        listbox.get_text_and_color(pos.y, pos.x),
                                    ),
                                );
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GLM_SET_UP_BUTTON => {
                        let mut links = self.listbox_links.unwrap_or_default();
                        links.up_button = data1 as WindowId;
                        self.listbox_links = Some(links);
                        return WindowMsgHandled::Handled;
                    }
                    GLM_SET_DOWN_BUTTON => {
                        let mut links = self.listbox_links.unwrap_or_default();
                        links.down_button = data1 as WindowId;
                        self.listbox_links = Some(links);
                        return WindowMsgHandled::Handled;
                    }
                    GLM_SET_SLIDER => {
                        let mut links = self.listbox_links.unwrap_or_default();
                        links.slider = data1 as WindowId;
                        self.listbox_links = Some(links);
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Handled;
                    }
                    GLM_SCROLL_BUFFER => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            let _ = listbox.scroll_buffer(data1);
                        }
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Handled;
                    }
                    GLM_UPDATE_DISPLAY => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            listbox.set_top_visible_entry(data1 as i32);
                        }
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Handled;
                    }
                    GLM_GET_ITEM_DATA => {
                        if let Some(WindowMsgPayload::CellPosition(pos)) = payload(data1) {
                            if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                                let _ = replace_payload(
                                    data2,
                                    WindowMsgPayload::ItemDataOpt(
                                        listbox.get_item_data_at(pos.y, pos.x).cloned(),
                                    ),
                                );
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GLM_SET_ITEM_DATA => {
                        if let Some(WindowMsgPayload::CellPosition(pos)) = payload(data1) {
                            if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                                let data = match payload(data2) {
                                    Some(WindowMsgPayload::ItemData(data)) => Some(data),
                                    Some(WindowMsgPayload::ItemDataOpt(data)) => data,
                                    _ => None,
                                };
                                let _ = listbox.set_item_data_at(pos.y, pos.x, data);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    _ => {}
                }
            }

            if let Some(links) = self.listbox_links {
                if msg == WindowMessage::GadgetSelected && data1 == links.up_button as WindowMsgData
                {
                    if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                        listbox.scroll_by(-1);
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }

                if msg == WindowMessage::GadgetSelected
                    && data1 == links.down_button as WindowMsgData
                {
                    if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                        listbox.scroll_by(1);
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }

                if (msg == WindowMessage::GadgetValueChanged
                    || msg == WindowMessage::User(GSM_SLIDER_TRACK))
                    && data1 == links.slider as WindowMsgData
                {
                    let max_offset = self
                        .list_box_mut()
                        .map(|listbox| {
                            let item_height = listbox.item_height().max(1) as usize;
                            let visible = (listbox.bounds().height as usize / item_height).max(1);
                            listbox.items().len().saturating_sub(visible)
                        })
                        .unwrap_or(0);
                    let slider_value = if msg == WindowMessage::User(GSM_SLIDER_TRACK) {
                        data2 as i32
                    } else if let Some(slider) = self.find_child_by_id(links.slider) {
                        match slider.borrow().widget() {
                            Some(WindowWidget::VerticalSlider(slider)) => slider.value(),
                            Some(WindowWidget::HorizontalSlider(slider)) => slider.value(),
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    let slider_value = slider_value.clamp(0, max_offset as i32) as usize;
                    if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                        listbox.set_scroll_offset(max_offset.saturating_sub(slider_value));
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }
            }
        }

        if matches!(
            self.widget,
            Some(WindowWidget::HorizontalSlider(_)) | Some(WindowWidget::VerticalSlider(_))
        ) {
            if let WindowMessage::User(code) = msg {
                match code {
                    GGM_LEFT_DRAG => {
                        return self.handle_slider_left_drag(data2);
                    }

                    GSM_SET_SLIDER => {
                        let new_pos = data1 as i32;
                        let mut update_thumb = false;
                        match self.widget.as_mut() {
                            Some(WindowWidget::HorizontalSlider(slider)) => {
                                let (min_val, max_val) = slider.range();
                                if (min_val..=max_val).contains(&new_pos) {
                                    slider.set_value(new_pos);
                                    update_thumb = true;
                                }
                            }
                            Some(WindowWidget::VerticalSlider(slider)) => {
                                let (min_val, max_val) = slider.range();
                                if (min_val..=max_val).contains(&new_pos) {
                                    slider.set_value(new_pos);
                                    update_thumb = true;
                                }
                            }
                            _ => {}
                        }
                        if update_thumb {
                            self.update_slider_thumb();
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GSM_SET_MIN_MAX => {
                        let min_val = data1 as i32;
                        let max_val = data2 as i32;
                        match self.widget.as_mut() {
                            Some(WindowWidget::HorizontalSlider(slider)) => {
                                slider.set_range(min_val, max_val);
                                slider.set_value(min_val);
                            }
                            Some(WindowWidget::VerticalSlider(slider)) => {
                                slider.set_range(min_val, max_val);
                                slider.set_value(min_val);
                            }
                            _ => {}
                        }
                        self.update_slider_thumb();
                        return WindowMsgHandled::Handled;
                    }
                    GGM_RESIZED => {
                        if let Some(thumb_id) = self.slider_thumb {
                            if let Some(thumb) = self.find_child_by_id(thumb_id) {
                                match self.widget.as_ref() {
                                    Some(WindowWidget::HorizontalSlider(_)) => {
                                        let _ =
                                            thumb.borrow_mut().set_size(GADGET_SIZE, data2 as i32);
                                    }
                                    Some(WindowWidget::VerticalSlider(_)) => {
                                        let _ =
                                            thumb.borrow_mut().set_size(data1 as i32, GADGET_SIZE);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    _ => {}
                }
            }
        }

        if matches!(self.widget, Some(WindowWidget::RadioButton(_))) {
            match msg {
                WindowMessage::Create | WindowMessage::Destroy => {
                    return WindowMsgHandled::Handled;
                }
                WindowMessage::InputFocus => {
                    if data1 == 0 {
                        self.set_hilite_state(false);
                    }
                    if !self.owner_is_self {
                        if let Some(owner) = self.get_owner() {
                            let _ = owner.borrow_mut().send_system_message(
                                WindowMessage::User(GGM_FOCUS_CHANGE),
                                data1,
                                self.id as WindowMsgData,
                            );
                        }
                    }
                    let _ = write_bool_payload(data2, true);
                    return WindowMsgHandled::Handled;
                }
                WindowMessage::User(code) if code == GBM_SET_SELECTION => {
                    let mut newly_selected = false;
                    let mut group_id = 0;
                    if let Some(WindowWidget::RadioButton(radio)) = self.widget.as_mut() {
                        group_id = radio.group_id();
                        if !radio.is_selected() {
                            radio.select();
                            self.inst_data.state.insert(WindowState::SELECTED);
                            newly_selected = true;
                        }
                    }
                    if newly_selected {
                        if group_id != 0 {
                            self.unselect_radio_peers_by_group(group_id);
                        }
                        if data1 != 0 && !self.owner_is_self {
                            if let Some(owner) = self.get_owner() {
                                let _ = owner.borrow_mut().send_system_message(
                                    WindowMessage::GadgetSelected,
                                    self.id as WindowMsgData,
                                    0,
                                );
                            }
                        }
                    }
                    return WindowMsgHandled::Handled;
                }
                _ => {}
            }
        }

        if matches!(self.widget, Some(WindowWidget::CheckBox(_))) {
            match msg {
                WindowMessage::Create | WindowMessage::Destroy => {
                    return WindowMsgHandled::Handled;
                }
                WindowMessage::User(code) if code == GBM_SET_SELECTION => {
                    return self.gadget_check_box_set_checked(data1 != 0);
                }
                _ => {}
            }
        }

        if matches!(self.widget, Some(WindowWidget::StaticText(_))) {
            match msg {
                WindowMessage::Create | WindowMessage::Destroy => {
                    return WindowMsgHandled::Handled;
                }
                WindowMessage::InputFocus => {
                    return WindowMsgHandled::Ignored;
                }
                _ => {}
            }
        }

        if matches!(self.widget, Some(WindowWidget::ProgressBar(_))) {
            if let WindowMessage::User(code) = msg {
                if code == GPM_SET_PROGRESS {
                    let progress = data1 as i32;
                    if (0..=100).contains(&progress) {
                        self.set_user_data(progress);
                        if let Some(WindowWidget::ProgressBar(progress_bar)) = self.widget.as_mut()
                        {
                            progress_bar.set_progress(progress as f32);
                        }
                    }
                    return WindowMsgHandled::Handled;
                }
            }
        }

        if matches!(self.widget, Some(WindowWidget::TabPane))
            && matches!(
                msg,
                WindowMessage::GadgetSelected
                    | WindowMessage::GadgetRightClick
                    | WindowMessage::GadgetMouseEntering
                    | WindowMessage::GadgetMouseLeaving
                    | WindowMessage::GadgetEditDone
            )
        {
            if let Some(parent) = self.get_parent() {
                return parent.borrow_mut().send_system_message(msg, data1, data2);
            }
        }

        if matches!(self.widget, Some(WindowWidget::TabControl(_)))
            && msg == WindowMessage::GadgetSelected
        {
            if let Some(parent) = self.get_parent() {
                return parent.borrow_mut().send_system_message(msg, data1, data2);
            }
        }

        if msg == WindowMessage::InputFocus {
            let focused = data1 != 0;
            let _ = write_bool_payload(data2, focused);
            let event = if focused {
                InputEvent::FocusGained
            } else {
                InputEvent::FocusLost
            };
            let messages = if let Some(widget) = self.widget.as_mut() {
                widget.handle_input(&event)
            } else {
                Vec::new()
            };
            if matches!(self.widget, Some(WindowWidget::TextEntry(_))) {
                if focused {
                    self.inst_data
                        .state
                        .insert(WindowState::SELECTED | WindowState::HILITED);
                } else {
                    self.inst_data
                        .state
                        .remove(WindowState::SELECTED | WindowState::HILITED);
                }
                let window_id = self.id;
                with_window_manager(|manager| {
                    if let Some(window) = manager.get_window_by_id(window_id) {
                        crate::gui::ime_manager::attach_or_detach_for_focus(window, focused);
                    }
                });
            } else if focused {
                if !matches!(self.widget, Some(WindowWidget::RadioButton(_))) {
                    self.set_hilite_state(true);
                }
            } else {
                self.set_hilite_state(false);
            }
            if matches!(self.widget, Some(WindowWidget::ComboBox(_))) {
                if !self.owner_is_self {
                    if let Some(owner) = self.get_owner() {
                        let _ = owner.borrow_mut().send_system_message(
                            WindowMessage::User(GGM_FOCUS_CHANGE),
                            data1,
                            self.id as WindowMsgData,
                        );
                    }
                }
                if let Some(links) = self.combobox_links {
                    if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                        let _ = with_payload(WindowMsgPayload::Bool(false), |token| {
                            edit_box.borrow_mut().send_system_message(
                                WindowMessage::InputFocus,
                                data1,
                                token,
                            )
                        });
                    }
                }
                let _ = write_bool_payload(data2, true);
                return WindowMsgHandled::Handled;
            }

            if !self.owner_is_self {
                if let Some(owner) = self.get_owner() {
                    let _ = owner.borrow_mut().send_system_message(
                        WindowMessage::User(GGM_FOCUS_CHANGE),
                        data1,
                        self.id as WindowMsgData,
                    );
                }
            }
            return if messages.is_empty() {
                WindowMsgHandled::Ignored
            } else {
                WindowMsgHandled::Handled
            };
        }

        WindowMsgHandled::Ignored
    }
}
