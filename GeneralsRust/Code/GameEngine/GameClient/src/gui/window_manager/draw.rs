//! Repaint passes, transitions, and default gadget draw callbacks.
#![allow(unused_imports)]

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
use crate::gui::{MAX_DRAW_DATA, MAX_WINDOWS};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use super::*;

/// C++ gadget creation (GameWindowManager.cpp:1857-1862) picks the device
/// image-draw function when the window carries WIN_STATUS_IMAGE — retail WNDs
/// author IMAGE on every button that receives runtime imagery
/// (ControlBar.wnd ButtonCommand01-14, ButtonQueue*, UnitUpgrade*, ...), even
/// while the draw-data IMAGE slots are still "NoImage". Selecting by authored
/// draw-data images alone left those buttons on the solid draw, painting the
/// authored `255 0 0 255` placeholder as solid red rectangles.
fn default_draw_uses_image(window: &GameWindow) -> bool {
    window.get_status().contains(WindowStatus::IMAGE)
        || window
            .instance_data()
            .enabled_draw_data
            .iter()
            .chain(window.instance_data().disabled_draw_data.iter())
            .chain(window.instance_data().hilite_draw_data.iter())
            .any(|draw| draw.image.is_some())
}

impl WindowManager {
    /// Draw all windows
    pub fn draw_all(&mut self) {
        // Match C++ WinRepaint ordering: top-level windows are stored head-first,
        // but repaint walks from tail to head in BELOW / normal / ABOVE passes.
        for window in self.root_windows.iter().rev() {
            let status = window.borrow().get_status();
            if status.contains(WindowStatus::BELOW) {
                self.draw_window_hierarchy(window);
            }
        }

        for window in self.root_windows.iter().rev() {
            let status = window.borrow().get_status();
            if !status.intersects(WindowStatus::ABOVE | WindowStatus::BELOW) {
                self.draw_window_hierarchy(window);
            }
        }

        for window in self.root_windows.iter().rev() {
            let status = window.borrow().get_status();
            if status.contains(WindowStatus::ABOVE) {
                self.draw_window_hierarchy(window);
            }
        }

        // C++ WinRepaint is BELOW / normal / ABOVE + transitions only.
        // Modal windows stay in root_windows (usually ABOVE); drawing them
        // again from modal_stack double-submits verts and can z-fight.
        self.transitions.draw();
    }

    /// Activate a transition group.
    pub fn transition_set_group(&mut self, group_name: &str, immediate: bool) {
        let window_lookup = self.window_by_id.clone();
        self.transitions
            .set_group(group_name, immediate, &window_lookup);
    }

    /// Reverse a transition group.
    pub fn transition_reverse(&mut self, group_name: &str) {
        let window_lookup = self.window_by_id.clone();
        self.transitions.reverse(group_name, &window_lookup);
    }

    /// Remove a transition group.
    pub fn transition_remove(&mut self, group_name: &str, skip_pending: bool) {
        self.transitions.remove(group_name, skip_pending);
    }

    /// Check if the current transition group has finished.
    pub fn transitions_finished(&self) -> bool {
        self.transitions.is_finished()
    }

    pub(crate) fn apply_draw_data_set(
        &self,
        window: &mut GameWindow,
        enabled: &[WindowDrawData],
        disabled: &[WindowDrawData],
        hilite: &[WindowDrawData],
    ) {
        for idx in 0..MAX_DRAW_DATA {
            if let Some(draw) = enabled.get(idx) {
                window.instance_data_mut().enabled_draw_data[idx] = draw.clone();
            }
            if let Some(draw) = disabled.get(idx) {
                window.instance_data_mut().disabled_draw_data[idx] = draw.clone();
            }
            if let Some(draw) = hilite.get(idx) {
                window.instance_data_mut().hilite_draw_data[idx] = draw.clone();
            }
        }
    }

    pub(crate) fn apply_default_draw_callback(&self, window: &mut GameWindow) {
        let has_image = default_draw_uses_image(window);

        let draw = match (window.widget(), has_image) {
            (Some(WindowWidget::PushButton(_)), true) => w3d_gadget_push_button_image_draw,
            (Some(WindowWidget::PushButton(_)), false) => w3d_gadget_push_button_draw,
            (Some(WindowWidget::TextEntry(_)), true) => w3d_gadget_text_entry_image_draw,
            (Some(WindowWidget::TextEntry(_)), false) => w3d_gadget_text_entry_draw,
            (Some(WindowWidget::ListBox(_)), true) => w3d_gadget_list_box_image_draw,
            (Some(WindowWidget::ListBox(_)), false) => w3d_gadget_list_box_draw,
            (Some(WindowWidget::StaticText(_)), true) => w3d_gadget_static_text_image_draw,
            (Some(WindowWidget::StaticText(_)), false) => w3d_gadget_static_text_draw,
            (Some(WindowWidget::ProgressBar(_)), true) => w3d_gadget_progress_bar_image_draw,
            (Some(WindowWidget::ProgressBar(_)), false) => w3d_gadget_progress_bar_draw,
            (Some(WindowWidget::CheckBox(_)), true) => w3d_gadget_check_box_image_draw,
            (Some(WindowWidget::CheckBox(_)), false) => w3d_gadget_check_box_draw,
            (Some(WindowWidget::RadioButton(_)), true) => w3d_gadget_radio_button_image_draw,
            (Some(WindowWidget::RadioButton(_)), false) => w3d_gadget_radio_button_draw,
            (Some(WindowWidget::VerticalSlider(_)), true) => w3d_gadget_vertical_slider_image_draw,
            (Some(WindowWidget::VerticalSlider(_)), false) => w3d_gadget_vertical_slider_draw,
            (Some(WindowWidget::HorizontalSlider(_)), true) => {
                w3d_gadget_horizontal_slider_image_draw
            }
            (Some(WindowWidget::HorizontalSlider(_)), false) => w3d_gadget_horizontal_slider_draw,
            (Some(WindowWidget::TabControl(_)), true) => w3d_gadget_tab_control_image_draw,
            (Some(WindowWidget::TabControl(_)), false) => w3d_gadget_tab_control_draw,
            (Some(WindowWidget::ComboBox(_)), true) => w3d_gadget_combo_box_image_draw,
            (Some(WindowWidget::ComboBox(_)), false) => w3d_gadget_combo_box_draw,
            // C++ W3DGameWindowManager::getDefaultDraw() returns W3DGameWinDefaultDraw,
            // so USER/[None] windows still render image/color draw data in the W3D path.
            _ => default_draw_callback,
        };
        window.set_draw_callback(draw);
    }

    pub(crate) fn apply_slider_draw_callback(&self, window: &mut GameWindow) {
        let has_image = window
            .instance_data()
            .enabled_draw_data
            .iter()
            .chain(window.instance_data().disabled_draw_data.iter())
            .chain(window.instance_data().hilite_draw_data.iter())
            .any(|draw| draw.image.is_some());

        let draw = if has_image {
            w3d_gadget_vertical_slider_image_draw
        } else {
            w3d_gadget_vertical_slider_draw
        };
        window.set_draw_callback(draw);
    }

    /// Draw window and its children recursively
    pub(crate) fn draw_window_hierarchy(&self, window: &Rc<RefCell<GameWindow>>) {
        self.draw_window_hierarchy_internal(window, false);
    }

    pub(crate) fn draw_window_hierarchy_internal(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        ancestor_hidden: bool,
    ) {
        let window_borrow = window.borrow();
        let name = window_borrow.get_name().to_string();
        let status = window_borrow.get_status();
        let see_thru = status.contains(WindowStatus::SEE_THRU);
        let effectively_hidden = ancestor_hidden || window_borrow.is_hidden();

        // Match C++ hierarchy semantics: a hidden parent suppresses its entire subtree.
        if effectively_hidden {
            return;
        }

        let border = status.contains(WindowStatus::BORDER) && !see_thru;
        let is_listbox = (window_borrow.get_style() & GWS_SCROLL_LISTBOX) != 0;

        if !see_thru {
            window_borrow.draw();
        }

        if is_listbox && border {
            window_borrow.draw_border_w3d();
        }

        // C++ drawWindow(): child = m_child; while(child->m_next) child = child->m_next;
        // for(; child; child = child->m_prev) drawWindow(child);
        // Our Vec is stored head-first, so reverse iteration matches tail-to-head repaint.
        for child in window_borrow.children().iter().rev() {
            self.draw_window_hierarchy_internal(child, effectively_hidden);
        }

        if !is_listbox && border {
            window_borrow.draw_border_w3d();
        }
    }
}
