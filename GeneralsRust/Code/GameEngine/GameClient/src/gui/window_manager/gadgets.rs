//! Gadget factories and script-created combo/slider/listbox/tab children.
#![allow(unused_imports)]

use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    RadioButtonGroup, StaticText, TabControl, TextEntry, VerticalSlider,
};
use crate::gui::game_window::*;
use crate::gui::w3d_gadget_draw::{
    w3d_cameo_movie_draw, w3d_clock_draw, w3d_command_bar_background_draw,
    w3d_command_bar_foreground_draw, w3d_command_bar_gen_exp_draw, w3d_command_bar_grid_draw,
    w3d_command_bar_help_popup_draw, w3d_command_bar_top_draw, w3d_credits_menu_draw,
    w3d_draw_map_preview, w3d_gadget_check_box_draw, w3d_gadget_check_box_image_draw,
    w3d_gadget_combo_box_draw, w3d_gadget_combo_box_image_draw, w3d_gadget_horizontal_slider_draw,
    w3d_gadget_horizontal_slider_image_draw, w3d_gadget_horizontal_slider_image_draw_a,
    w3d_gadget_horizontal_slider_image_draw_b, w3d_gadget_list_box_draw,
    w3d_gadget_list_box_image_draw, w3d_gadget_progress_bar_draw,
    w3d_gadget_progress_bar_image_draw, w3d_gadget_progress_bar_image_draw_a,
    w3d_gadget_push_button_draw, w3d_gadget_push_button_image_draw, w3d_gadget_radio_button_draw,
    w3d_gadget_radio_button_image_draw, w3d_gadget_static_text_draw,
    w3d_gadget_static_text_image_draw, w3d_gadget_tab_control_draw,
    w3d_gadget_tab_control_image_draw, w3d_gadget_text_entry_draw,
    w3d_gadget_text_entry_image_draw, w3d_gadget_vertical_slider_draw,
    w3d_gadget_vertical_slider_image_draw, w3d_left_hud_draw,
    w3d_main_menu_button_drop_shadow_draw, w3d_main_menu_draw, w3d_main_menu_four_draw,
    w3d_main_menu_map_border, w3d_main_menu_random_text_draw, w3d_metal_bar_menu_draw, w3d_no_draw,
    w3d_power_draw, w3d_power_draw_a, w3d_right_hud_draw, w3d_shell_menu_scheme_draw,
    w3d_thin_border_draw,
};
use crate::gui::window_script::{
    TabControlData as ScriptTabControlData, WindowDefinition, WindowLayoutDefinition,
    parse_window_script,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use super::*;

fn nonempty_draw<'a>(
    window: &'a [WindowDrawData],
    layout: &'a [WindowDrawData],
) -> &'a [WindowDrawData] {
    if !window.is_empty() { window } else { layout }
}

impl WindowManager {
    pub(crate) fn create_default_tab_panes(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
    ) -> WindowResult<()> {
        let (pane_x, pane_y, pane_width, pane_height) = self.compute_tab_pane_rect(window);

        for pane_index in 0..crate::gui::gadgets::tabcontrol::NUM_TAB_PANES {
            let pane_id = generate_window_id();
            let pane = self.create_window_with_id_internal(
                Some(window),
                pane_x,
                pane_y,
                pane_width,
                pane_height,
                pane_id,
                false,
            )?;
            {
                let mut pane_mut = pane.borrow_mut();
                if let Some(layout) = window.borrow().get_layout() {
                    pane_mut.set_layout(Some(&layout));
                }
                let data = pane_mut.instance_data_mut();
                data.style |= GWS_TAB_PANE;
                data.decorated_name = format!("Pane {}", pane_index);
                pane_mut.set_widget(WindowWidget::TabPane);
                pane_mut.enable(window.borrow().is_enabled())?;
            }
        }

        Ok(())
    }

    pub(crate) fn resize_tab_panes(&self, window: &Rc<RefCell<GameWindow>>) {
        let (pane_x, pane_y, pane_width, pane_height) = self.compute_tab_pane_rect(window);
        let panes: Vec<Rc<RefCell<GameWindow>>> = window
            .borrow()
            .children()
            .iter()
            .rev()
            .filter(|child| {
                let child = child.borrow();
                (child.get_style() & GWS_TAB_PANE) != 0
            })
            .cloned()
            .collect();

        for pane in panes {
            let mut pane_mut = pane.borrow_mut();
            let _ = pane_mut.set_size(pane_width, pane_height);
            let _ = pane_mut.set_position(pane_x, pane_y);
        }
    }

    pub(crate) fn compute_tab_pane_rect(
        &self,
        window: &Rc<RefCell<GameWindow>>,
    ) -> (i32, i32, i32, i32) {
        let window_ref = window.borrow();
        let (win_width, win_height) = window_ref.get_size();
        let (win_width, win_height) = (win_width, win_height);
        let mut tab_edge = crate::gui::gadgets::tabcontrol::TP_TOP_SIDE;
        let mut tab_width = 0;
        let mut tab_height = 0;
        let mut pane_border = 0;

        if let Some(WindowWidget::TabControl(tab_control)) = window_ref.widget() {
            tab_edge = tab_control.tab_edge();
            tab_width = tab_control.tab_width_px();
            tab_height = tab_control.tab_height_px();
            pane_border = tab_control.pane_border();
        }

        let mut width = win_width - (2 * pane_border);
        let mut height = win_height - (2 * pane_border);

        if tab_edge == crate::gui::gadgets::tabcontrol::TP_TOP_SIDE
            || tab_edge == crate::gui::gadgets::tabcontrol::TP_BOTTOM_SIDE
        {
            height -= tab_height;
        }
        if tab_edge == crate::gui::gadgets::tabcontrol::TP_LEFT_SIDE
            || tab_edge == crate::gui::gadgets::tabcontrol::TP_RIGHT_SIDE
        {
            width -= tab_width;
        }

        let mut x = pane_border;
        let mut y = pane_border;
        if tab_edge == crate::gui::gadgets::tabcontrol::TP_LEFT_SIDE {
            x += tab_width;
        }
        if tab_edge == crate::gui::gadgets::tabcontrol::TP_TOP_SIDE {
            y += tab_height;
        }

        (x, y, width.max(0), height.max(0))
    }

    pub(crate) fn create_combo_box_children(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        layout: &WindowLayoutDefinition,
        window_def: &WindowDefinition,
    ) -> WindowResult<()> {
        let (width, height) = window.borrow().get_size();
        let mut status = window.borrow().get_status();
        status.remove(WindowStatus::BORDER);
        status.remove(WindowStatus::HIDDEN);
        let is_editable = window_def
            .combo_box_data
            .as_ref()
            .map(|data| data.is_editable)
            .unwrap_or(false);

        let button_width = 21;
        let button_height = height;

        let drop_down_id = generate_window_id();
        let drop_down = self.create_window_with_id_internal(
            Some(window),
            (width - button_width).max(0),
            0,
            button_width,
            button_height,
            drop_down_id,
            false,
        )?;
        {
            let mut drop_mut = drop_down.borrow_mut();
            drop_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            drop_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                drop_down_id as u32,
                0,
                0,
                button_width as u32,
                height.max(0) as u32,
            )));
            drop_mut.set_status_exact(status | WindowStatus::ACTIVE | WindowStatus::ENABLED);
            if let Some(font) = window.borrow().get_font().cloned() {
                drop_mut.set_font(font);
            }
            drop_mut.set_tooltip(window.borrow().get_tooltip());
            drop_mut.instance_data_mut().tooltip_delay = window.borrow().get_tooltip_delay();
            let (en, dis, hi) = (
                nonempty_draw(
                    &window_def.combo_dropdown_enabled_draw_data,
                    &layout.combo_dropdown_enabled_draw_data,
                ),
                nonempty_draw(
                    &window_def.combo_dropdown_disabled_draw_data,
                    &layout.combo_dropdown_disabled_draw_data,
                ),
                nonempty_draw(
                    &window_def.combo_dropdown_hilite_draw_data,
                    &layout.combo_dropdown_hilite_draw_data,
                ),
            );
            self.apply_draw_data_set(&mut drop_mut, en, dis, hi);
            self.apply_default_draw_callback(&mut drop_mut);
        }

        let edit_id = generate_window_id();
        let edit_width = (width - button_width).max(0);
        let edit = self.create_window_with_id_internal(
            Some(window),
            0,
            0,
            edit_width,
            height,
            edit_id,
            false,
        )?;
        {
            let mut edit_mut = edit.borrow_mut();
            edit_mut.instance_data_mut().style |= GWS_ENTRY_FIELD;
            edit_mut.set_widget(WindowWidget::TextEntry(TextEntry::new(
                edit_id as u32,
                0,
                0,
                edit_width as u32,
                height.max(0) as u32,
            )));
            let mut edit_status = status;
            if !is_editable {
                edit_status |= WindowStatus::NO_INPUT;
            }
            edit_mut.set_status_exact(edit_status);
            if let Some(font) = window.borrow().get_font().cloned() {
                edit_mut.set_font(font);
            }
            edit_mut.set_tooltip(window.borrow().get_tooltip());
            edit_mut.instance_data_mut().tooltip_delay = window.borrow().get_tooltip_delay();
            if let Some(WindowWidget::TextEntry(entry)) = edit_mut.widget_mut() {
                entry.set_draw_text_from_start(!is_editable);
            }
            if let Some(data) = window_def.combo_box_data.as_ref() {
                if let Some(WindowWidget::TextEntry(entry)) = edit_mut.widget_mut() {
                    let validation = if data.ascii_only {
                        crate::gui::gadgets::ValidationMode::AsciiOnly
                    } else if data.letters_and_numbers {
                        crate::gui::gadgets::ValidationMode::AlphanumericOnly
                    } else {
                        crate::gui::gadgets::ValidationMode::None
                    };
                    entry.set_validation(validation);
                    if data.max_chars > 0 {
                        entry.set_max_length(data.max_chars);
                    }
                }
            }
            let (en, dis, hi) = (
                nonempty_draw(
                    &window_def.combo_edit_enabled_draw_data,
                    &layout.combo_edit_enabled_draw_data,
                ),
                nonempty_draw(
                    &window_def.combo_edit_disabled_draw_data,
                    &layout.combo_edit_disabled_draw_data,
                ),
                nonempty_draw(
                    &window_def.combo_edit_hilite_draw_data,
                    &layout.combo_edit_hilite_draw_data,
                ),
            );
            self.apply_draw_data_set(&mut edit_mut, en, dis, hi);
            self.apply_default_draw_callback(&mut edit_mut);
        }

        let list_id = generate_window_id();
        let list = self.create_window_with_id_internal(
            Some(window),
            0,
            height,
            width,
            height,
            list_id,
            false,
        )?;
        {
            let mut list_mut = list.borrow_mut();
            list_mut.instance_data_mut().style |= GWS_SCROLL_LISTBOX;
            list_mut.set_widget(WindowWidget::ListBox(ListBox::new(
                list_id as u32,
                0,
                height,
                width.max(0) as u32,
                height.max(0) as u32,
            )));
            let mut list_status = status;
            list_status.remove(WindowStatus::IMAGE);
            list_mut.set_status_exact(list_status | WindowStatus::ABOVE | WindowStatus::ONE_LINE);
            list_mut.hide(true)?;
            if let Some(font) = window.borrow().get_font().cloned() {
                list_mut.set_font(font);
            }
            list_mut.set_tooltip(window.borrow().get_tooltip());
            list_mut.instance_data_mut().tooltip_delay = window.borrow().get_tooltip_delay();
            if let Some(WindowWidget::ListBox(listbox)) = list_mut.widget_mut() {
                listbox.set_max_length(10);
                listbox.set_auto_purge(false);
                listbox.set_auto_scroll(false);
                listbox.set_scroll_if_at_end(false);
                listbox.set_force_select(true);
                listbox.set_selection_mode(crate::gui::gadgets::SelectionMode::Single);
                listbox.set_columns(1);
                listbox.set_audio_feedback(true);
            }
            let (en, dis, hi) = (
                nonempty_draw(
                    &window_def.combo_list_enabled_draw_data,
                    &layout.combo_list_enabled_draw_data,
                ),
                nonempty_draw(
                    &window_def.combo_list_disabled_draw_data,
                    &layout.combo_list_disabled_draw_data,
                ),
                nonempty_draw(
                    &window_def.combo_list_hilite_draw_data,
                    &layout.combo_list_hilite_draw_data,
                ),
            );
            self.apply_draw_data_set(&mut list_mut, en, dis, hi);
            self.apply_default_draw_callback(&mut list_mut);
        }

        self.create_listbox_scrollbar_children(&list, layout, Some(window_def))?;

        window
            .borrow_mut()
            .set_combobox_links(crate::gui::game_window::ComboBoxLinks {
                drop_down: drop_down_id,
                edit_box: edit_id,
                list_box: list_id,
            });

        Ok(())
    }

    pub(crate) fn create_slider_thumb_child(
        &mut self,
        slider: &Rc<RefCell<GameWindow>>,
        layout: &WindowLayoutDefinition,
    ) -> WindowResult<()> {
        self.create_slider_thumb_child_with_window(slider, layout, None)
    }

    pub(crate) fn create_slider_thumb_child_with_window(
        &mut self,
        slider: &Rc<RefCell<GameWindow>>,
        layout: &WindowLayoutDefinition,
        window_def: Option<&crate::gui::window_script::WindowDefinition>,
    ) -> WindowResult<()> {
        // C++ gogoGadgetSlider always creates a thumb (ENABLED | DRAGABLE).
        // Prefer per-WINDOW SLIDERTHUMB* draw data; fall back to layout-global.
        let (enabled, disabled, hilite) = match window_def {
            Some(window)
                if !window.slider_thumb_enabled_draw_data.is_empty()
                    || !window.slider_thumb_disabled_draw_data.is_empty()
                    || !window.slider_thumb_hilite_draw_data.is_empty() =>
            {
                (
                    window.slider_thumb_enabled_draw_data.as_slice(),
                    window.slider_thumb_disabled_draw_data.as_slice(),
                    window.slider_thumb_hilite_draw_data.as_slice(),
                )
            }
            _ => (
                layout.slider_thumb_enabled_draw_data.as_slice(),
                layout.slider_thumb_disabled_draw_data.as_slice(),
                layout.slider_thumb_hilite_draw_data.as_slice(),
            ),
        };

        let (width, _height) = slider.borrow().get_size();
        let is_horizontal = (slider.borrow().get_style() & GWS_HORZ_SLIDER) != 0;
        let (thumb_w, thumb_h) = if is_horizontal { (13, 16) } else { (width, 16) };
        let thumb_y = if is_horizontal { 10 } else { 0 };

        let mut status = slider.borrow().get_status();
        status.remove(WindowStatus::BORDER | WindowStatus::HIDDEN | WindowStatus::NO_INPUT);
        status.insert(WindowStatus::ACTIVE | WindowStatus::ENABLED | WindowStatus::DRAGABLE);

        let thumb_id = generate_window_id();
        let thumb = self.create_window_with_id_internal(
            Some(slider),
            0,
            thumb_y,
            thumb_w,
            thumb_h,
            thumb_id,
            false,
        )?;
        {
            let mut thumb_mut = thumb.borrow_mut();
            thumb_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            thumb_mut.set_status_exact(status);
            thumb_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                thumb_id as u32,
                0,
                0,
                thumb_w as u32,
                thumb_h as u32,
            )));
            self.apply_draw_data_set(&mut thumb_mut, enabled, disabled, hilite);
            self.apply_default_draw_callback(&mut thumb_mut);
        }

        slider.borrow_mut().set_slider_thumb(thumb_id);
        slider.borrow_mut().update_slider_thumb();

        Ok(())
    }

    pub(crate) fn create_listbox_scrollbar_children(
        &mut self,
        listbox: &Rc<RefCell<GameWindow>>,
        layout: &WindowLayoutDefinition,
        window_def: Option<&WindowDefinition>,
    ) -> WindowResult<()> {
        let empty: &[WindowDrawData] = &[];
        let (up_en, up_dis, up_hi) = (
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_enabled_up_button_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_enabled_up_button_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_disabled_up_button_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_disabled_up_button_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_hilite_up_button_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_hilite_up_button_draw_data,
            ),
        );
        let (dn_en, dn_dis, dn_hi) = (
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_enabled_down_button_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_enabled_down_button_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_disabled_down_button_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_disabled_down_button_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_hilite_down_button_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_hilite_down_button_draw_data,
            ),
        );
        let (sl_en, sl_dis, sl_hi) = (
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_enabled_slider_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_enabled_slider_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_disabled_slider_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_disabled_slider_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.listbox_hilite_slider_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.listbox_hilite_slider_draw_data,
            ),
        );
        let (th_en, th_dis, th_hi) = (
            nonempty_draw(
                window_def
                    .map(|w| w.slider_thumb_enabled_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.slider_thumb_enabled_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.slider_thumb_disabled_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.slider_thumb_disabled_draw_data,
            ),
            nonempty_draw(
                window_def
                    .map(|w| w.slider_thumb_hilite_draw_data.as_slice())
                    .unwrap_or(empty),
                &layout.slider_thumb_hilite_draw_data,
            ),
        );
        let (width, height) = listbox.borrow().get_size();
        let button_width = 21;
        let button_height = 22;
        let has_title = !listbox.borrow().get_text().is_empty();
        let font_height = if has_title {
            listbox
                .borrow()
                .get_font()
                .map(|font| self.win_font_height(font))
                .unwrap_or(12)
        } else {
            0
        };
        let top = if has_title { font_height + 1 } else { 0 };
        let bottom = if has_title {
            height - (font_height + 1)
        } else {
            height
        };

        let mut status = listbox.borrow().get_status();
        status.remove(WindowStatus::BORDER | WindowStatus::HIDDEN | WindowStatus::NO_INPUT);
        status.insert(WindowStatus::ACTIVE | WindowStatus::ENABLED);

        let up_id = generate_window_id();
        let up_button = self.create_window_with_id_internal(
            Some(listbox),
            width - button_width - 2,
            top + 2,
            button_width,
            button_height,
            up_id,
            false,
        )?;
        {
            let mut up_mut = up_button.borrow_mut();
            up_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            up_mut.set_status_exact(status);
            let mut button = PushButton::new(
                up_id as u32,
                0,
                0,
                button_width as u32,
                button_height as u32,
            );
            button.set_triggers_on_mouse_down(true);
            up_mut.set_widget(WindowWidget::PushButton(button));
            self.apply_draw_data_set(&mut up_mut, up_en, up_dis, up_hi);
            self.apply_default_draw_callback(&mut up_mut);
        }

        let down_id = generate_window_id();
        let down_button = self.create_window_with_id_internal(
            Some(listbox),
            width - button_width - 2,
            top + bottom - button_height - 2,
            button_width,
            button_height,
            down_id,
            false,
        )?;
        {
            let mut down_mut = down_button.borrow_mut();
            down_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            down_mut.set_status_exact(status);
            let mut button = PushButton::new(
                down_id as u32,
                0,
                0,
                button_width as u32,
                button_height as u32,
            );
            button.set_triggers_on_mouse_down(true);
            down_mut.set_widget(WindowWidget::PushButton(button));
            self.apply_draw_data_set(&mut down_mut, dn_en, dn_dis, dn_hi);
            self.apply_default_draw_callback(&mut down_mut);
        }

        let slider_id = generate_window_id();
        let slider_height = (bottom - (2 * button_height) - 6).max(0);
        let slider = self.create_window_with_id_internal(
            Some(listbox),
            width - button_width - 2,
            top + button_height + 3,
            button_width,
            slider_height,
            slider_id,
            false,
        )?;
        {
            let mut slider_mut = slider.borrow_mut();
            slider_mut.instance_data_mut().style |= GWS_VERT_SLIDER;
            slider_mut.set_status_exact(status);
            slider_mut.set_widget(WindowWidget::VerticalSlider(VerticalSlider::new(
                slider_id as u32,
                0,
                0,
                button_width as u32,
                slider_height as u32,
            )));
            self.apply_draw_data_set(&mut slider_mut, sl_en, sl_dis, sl_hi);
            self.apply_slider_draw_callback(&mut slider_mut);
        }

        let mut thumb_id = None;
        if !th_en.is_empty() || !th_dis.is_empty() || !th_hi.is_empty() {
            let thumb_window_id = generate_window_id();
            let thumb = self.create_window_with_id_internal(
                Some(&slider),
                0,
                0,
                button_width,
                16,
                thumb_window_id,
                false,
            )?;
            {
                let mut thumb_mut = thumb.borrow_mut();
                thumb_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
                thumb_mut.set_status_exact(status);
                thumb_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                    thumb_window_id as u32,
                    0,
                    0,
                    button_width as u32,
                    16,
                )));
                self.apply_draw_data_set(&mut thumb_mut, th_en, th_dis, th_hi);
                self.apply_default_draw_callback(&mut thumb_mut);
            }
            thumb_id = Some(thumb_window_id);
        }

        listbox
            .borrow_mut()
            .set_listbox_links(crate::gui::game_window::ListBoxLinks {
                up_button: up_id,
                down_button: down_id,
                slider: slider_id,
                thumb: thumb_id,
            });
        listbox.borrow_mut().update_listbox_scrollbar();

        Ok(())
    }

    /// PARITY_NOTE: C++ uses `TheWindowManager->getMessageBox()` with explicit
    /// yes/no button callbacks. This Rust version creates the window directly
    /// and wires up the callbacks via user data, matching the observable behavior.
    pub fn gogo_message_box(
        &mut self,
        title: &str,
        body: &str,
        yes_cb: Option<Box<dyn Fn()>>,
        no_cb: Option<Box<dyn Fn()>>,
    ) -> Option<WindowId> {
        let (screen_w, screen_h) = self.screen_size;
        let box_w = (screen_w as f32 * 0.4) as i32;
        let box_h = (screen_h as f32 * 0.25) as i32;
        let box_x = (screen_w - box_w) / 2;
        let box_y = (screen_h - box_h) / 2;

        let window = self.create_window(None, box_x, box_y, box_w, box_h).ok()?;
        let window_id = window.borrow().get_id();

        {
            let mut wm = window.borrow_mut();
            wm.set_name("MessageBox");
            let _ = wm.set_text(body);
            wm.instance_data_mut().text_label = title.to_string();
            wm.set_status_exact(
                WindowStatus::ACTIVE
                    | WindowStatus::ENABLED
                    | WindowStatus::ABOVE
                    | WindowStatus::NO_FOCUS,
            );
            if let Some(cb) = yes_cb {
                wm.set_user_data(cb);
            }
            wm.set_system_callback(default_system_callback);
            wm.set_draw_callback(default_draw_callback);
        }

        let _ = self.set_modal(window);
        Some(window_id)
    }

    pub fn gogo_gadget_push_button(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_PUSH_BUTTON;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::PushButton(PushButton::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_checkbox(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_CHECK_BOX;
            let gadget_id = window_id as u32;
            let box_size = size.0.min(size.1).max(0) as u32;
            wm.set_widget(WindowWidget::CheckBox(CheckBox::new(
                gadget_id, pos.0, pos.1, box_size,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_radio_button(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_RADIO_BUTTON;
            let gadget_id = window_id as u32;
            let group = RadioButtonGroup::new(gadget_id);
            let btn_size = size.0.min(size.1).max(0) as u32;
            wm.set_widget(WindowWidget::RadioButton(RadioButton::new(
                gadget_id, pos.0, pos.1, btn_size, group,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_tab_control(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_TAB_CONTROL;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::TabControl(TabControl::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_list_box(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_SCROLL_LISTBOX;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::ListBox(ListBox::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_slider(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_HORZ_SLIDER;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::HorizontalSlider(HorizontalSlider::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_progress_bar(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_PROGRESS_BAR;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::ProgressBar(ProgressBar::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_static_text(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_STATIC_TEXT;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::StaticText(StaticText::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_text_entry(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_ENTRY_FIELD;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::TextEntry(TextEntry::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_combo_box(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_COMBO_BOX;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::ComboBox(ComboBox::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }
}
