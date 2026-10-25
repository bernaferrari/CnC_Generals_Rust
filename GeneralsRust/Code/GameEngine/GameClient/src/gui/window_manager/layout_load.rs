//! Layout construction and .wnd script instantiation.
#![allow(unused_imports)]

use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    RadioButtonGroup, StaticText, TabControl, TextEntry, VerticalSlider,
};
use crate::gui::game_window::*;
use crate::gui::header_template::get_header_template_manager;
use crate::gui::window_script::{
    TabControlData as ScriptTabControlData, WindowDefinition, WindowLayoutDefinition,
    parse_window_script,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Instant;

use super::*;

/// Weak registrations and plain group data captured for one fallible load.
/// None of these values becomes another mutable UI owner.
struct LayoutRegistrationBefore {
    lookup: HashMap<WindowId, Weak<RefCell<GameWindow>>>,
    tabs: Vec<Weak<RefCell<GameWindow>>>,
    radio_groups: Vec<(
        u32,
        crate::gui::gadgets::radiobutton::RadioButtonGroupBefore,
    )>,
}

impl WindowManager {
    /// Load a window layout file and return the first window instance.
    pub fn load_window(
        &mut self,
        filename: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        let layout_info = self.create_windows_from_script(filename, effects)?;
        layout_info
            .windows
            .first()
            .cloned()
            .ok_or(WindowError::InvalidParameter)
    }

    /// Create a window layout
    pub fn create_layout(&mut self, filename: String) -> Rc<RefCell<WindowLayout>> {
        let layout = Rc::new(RefCell::new(WindowLayout::new(filename)));
        layout.borrow_mut().set_self_handle(&layout);
        self.layouts.push(layout.clone());
        layout
    }

    /// Create a window layout, populate it from script, and return the layout with info.
    pub fn create_layout_with_windows(
        &mut self,
        filename: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<(Rc<RefCell<WindowLayout>>, WindowLayoutInfo)> {
        self.create_layout_with_windows_with_effects(filename, effects)
    }

    /// Remove a layout after destroying its windows.
    pub fn destroy_layout(
        &mut self,
        layout: &Rc<RefCell<WindowLayout>>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.destroy_layout_with_effects(layout, effects)
    }

    /// Load windows from script and create window instances.
    pub fn create_windows_from_script(
        &mut self,
        filename: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<WindowLayoutInfo> {
        self.create_windows_from_script_with_effects(filename, effects)
    }

    pub fn create_layout_with_windows_with_effects(
        &mut self,
        filename: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<(Rc<RefCell<WindowLayout>>, WindowLayoutInfo)> {
        let path = resolve_window_script_path(filename)?;
        let definition = parse_window_script(&path).map_err(|_| WindowError::GeneralFailure)?;
        self.instantiate_layout_definition(filename, &definition, effects)
    }
    pub fn create_windows_from_script_with_effects(
        &mut self,
        filename: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<WindowLayoutInfo> {
        self.create_layout_with_windows_with_effects(filename, effects)
            .map(|(_, info)| info)
    }
    pub fn destroy_layout_with_effects(
        &mut self,
        layout: &Rc<RefCell<WindowLayout>>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let windows = layout.borrow_mut().take_windows();
        for window in windows {
            let _ = self.destroy_window(window);
        }
        self.layouts.retain(|entry| !Rc::ptr_eq(entry, layout));
        self.flush_destroy_queue_with_effects(effects);
    }

    fn capture_layout_registrations(&self) -> LayoutRegistrationBefore {
        LayoutRegistrationBefore {
            lookup: self.window_by_id.clone(),
            tabs: self.tab_list.clone(),
            radio_groups: self
                .radio_groups
                .iter()
                .map(|(id, group)| (*id, group.capture_layout_beforeimage()))
                .collect(),
        }
    }

    #[cfg(test)]
    fn rollback_failed_layout(
        &mut self,
        layout: Rc<RefCell<WindowLayout>>,
        info: WindowLayoutInfo,
        before: LayoutRegistrationBefore,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.rollback_failed_layout_with_effects(layout, info, before, None, effects)
    }
    fn rollback_failed_layout_with_effects(
        &mut self,
        layout: Rc<RefCell<WindowLayout>>,
        info: WindowLayoutInfo,
        before: LayoutRegistrationBefore,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        // Isolate unrelated deferred deletion while destroying only the
        // candidate tree through the ordinary local-manager destruction path.
        let mut candidate_windows = layout.borrow().windows().to_vec();
        let mut index = 0;
        while index < candidate_windows.len() {
            let children = candidate_windows[index].borrow().children().to_vec();
            candidate_windows.extend(children);
            index += 1;
        }
        let mut deferred = std::mem::take(&mut self.destroy_queue);
        match effects {
            _ => self.destroy_layout_with_effects(&layout, effects),
        };
        deferred.retain(|window| {
            !candidate_windows
                .iter()
                .any(|candidate| Rc::ptr_eq(window, candidate))
        });
        self.destroy_queue.extend(deferred);

        // GWM_DESTROY callbacks have run. Reclaim widgets now, even if a
        // callback retained an Rc to its already-destroyed candidate window.
        // RadioButton owns its widget value directly (not through another Rc),
        // so a later window drop cannot remove a restored original member ID.
        for window in &candidate_windows {
            drop(window.borrow_mut().widget.take());
        }
        drop(info);
        drop(candidate_windows);
        drop(layout);

        self.window_by_id = before.lookup;
        self.tab_list = before.tabs;
        self.radio_groups
            .retain(|id, _| before.radio_groups.iter().any(|(old, _)| old == id));
        for (id, state) in before.radio_groups {
            self.radio_groups
                .get(&id)
                .expect("existing group survives candidate construction")
                .restore_layout_beforeimage(state);
        }
    }

    /// One creation kernel for both script APIs. Failed Rust construction has
    /// no returned layout handle, so remove its partial tree before returning.
    /// Successful window/Create/ScriptCreate order is unchanged.
    fn instantiate_layout_definition(
        &mut self,
        filename: &str,
        layout_def: &WindowLayoutDefinition,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<(Rc<RefCell<WindowLayout>>, WindowLayoutInfo)> {
        // C++ winCreateFromScript returns no first window for an empty file;
        // WindowLayout::load then fails without keeping an empty layout.
        if layout_def.windows.is_empty() {
            return Err(WindowError::InvalidParameter);
        }
        // Detached weak registration observations only. A candidate can reuse
        // an existing named ID, and normal window destruction removes that ID.
        // Retain the previous registration rather than losing another layout's
        // lookup or leaving candidate TABSTOP entries after a failed load.
        let before = self.capture_layout_registrations();
        let layout = self.create_layout(filename.to_string());
        {
            let mut layout_mut = layout.borrow_mut();
            layout_mut.default_text_color = layout_def.default_text_color;
            layout_mut.default_font = layout_def.default_font.clone();
            self.bind_layout_callbacks(&mut layout_mut, layout_def);
        }
        let mut info = WindowLayoutInfo {
            version: layout_def.version,
            init_callback_name: layout_def.init_callback.clone(),
            update_callback_name: layout_def.update_callback.clone(),
            shutdown_callback_name: layout_def.shutdown_callback.clone(),
            windows: Vec::new(),
        };
        for window_def in &layout_def.windows {
            if let Err(error) = self.create_window_from_definition_with_effects(
                window_def,
                None,
                &layout,
                layout_def,
                &mut info,
                &mut *effects,
            ) {
                self.rollback_failed_layout_with_effects(layout, info, before, &mut *effects);
                return Err(error);
            }
        }
        Ok((layout, info))
    }

    pub(crate) fn create_window_from_definition(
        &mut self,
        window_def: &WindowDefinition,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        layout: &Rc<RefCell<WindowLayout>>,
        layout_def: &WindowLayoutDefinition,
        info: &mut WindowLayoutInfo,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        self.create_window_from_definition_with_effects(
            window_def, parent, layout, layout_def, info, effects,
        )
    }
    fn create_window_from_definition_with_effects(
        &mut self,
        window_def: &WindowDefinition,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        layout: &Rc<RefCell<WindowLayout>>,
        layout_def: &WindowLayoutDefinition,
        info: &mut WindowLayoutInfo,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        let (x, y, width, height) = self.resolve_window_rect(window_def, parent);
        log::debug!(
            "Creating window '{}' type={:?} rect=({}, {}, {}, {}) parent={}",
            window_def.name,
            window_def.window_type,
            x,
            y,
            width,
            height,
            parent
                .map(|p| p.borrow().get_name().to_string())
                .unwrap_or_else(|| "<root>".to_string())
        );
        let window_id = if window_def.name.is_empty() {
            self.generate_window_id()
        } else {
            NameKeyGenerator::name_to_key(&window_def.name) as WindowId
        };
        let window = self
            .create_window_with_id_internal(parent, x, y, width, height, window_id, false, effects)
            .map_err(|err| {
                log::error!(
                    "Failed to create window '{}' type={:?} rect=({}, {}, {}, {}): {:?}",
                    window_def.name,
                    window_def.window_type,
                    x,
                    y,
                    width,
                    height,
                    err
                );
                err
            })?;
        let has_tab_pane_child = window_def.children.iter().any(|child| {
            let style = child.style | style_for_window_type(&child.window_type);
            (style & GWS_TAB_PANE) != 0
        });
        {
            let mut window_mut = window.borrow_mut();
            window_mut.set_layout(Some(layout));
            let data = window_mut.instance_data_mut();
            data.style = window_def.style | style_for_window_type(&window_def.window_type);
            data.decorated_name = window_def.name.clone();
            data.text_label = window_def.text_label.clone();
            data.header_template = window_def.header_template.clone();
            data.tooltip_delay = window_def.tooltip_delay;
            data.text = window_def.text.clone();
            data.tooltip = window_def.tooltip.clone();
            data.enabled_text = window_def.enabled_text.clone();
            data.disabled_text = window_def.disabled_text.clone();
            data.hilite_text = window_def.hilite_text.clone();
            // C++ parseImageOffset writes instData->m_imageOffset (GameWindowManagerScript.cpp:545-556).
            data.image_offset = crate::gui::game_window::Point2D {
                x: window_def.image_offset.0,
                y: window_def.image_offset.1,
            };
            if let Some(font) = window_def.font.clone() {
                data.font = Some(font);
            } else if let Some(default_font) = layout.borrow().default_font.clone() {
                data.font = Some(default_font);
            }
            if !data.header_template.is_empty() {
                if let Some(font) =
                    get_header_template_manager().get_font_from_template(&data.header_template)
                {
                    data.font = Some(font);
                }
            }
            for (idx, draw) in window_def.enabled_draw_data.iter().enumerate() {
                if idx < data.enabled_draw_data.len() {
                    data.enabled_draw_data[idx] = draw.clone();
                }
            }
            for (idx, draw) in window_def.disabled_draw_data.iter().enumerate() {
                if idx < data.disabled_draw_data.len() {
                    data.disabled_draw_data[idx] = draw.clone();
                }
            }
            for (idx, draw) in window_def.hilite_draw_data.iter().enumerate() {
                if idx < data.hilite_draw_data.len() {
                    data.hilite_draw_data[idx] = draw.clone();
                }
            }
            if let Some(parent_window) = parent {
                data.owner = Some(Rc::downgrade(parent_window));
            }
            if let Some(widget) = create_widget_for_style(
                &mut self.radio_groups,
                window_def,
                window_mut.get_id(),
                x,
                y,
                width,
                height,
            ) {
                window_mut.set_widget(widget);
            }
            apply_window_text(&mut window_mut, window_def);
            apply_window_tooltip(&mut window_mut, window_def);
            window_mut.set_status_exact(window_def.status);
            apply_window_status_to_widget(&mut window_mut);
            apply_window_widget_data(&mut window_mut, window_def);
            self.bind_window_callbacks(&mut window_mut, window_def);
            if window_def.draw_callback.is_empty()
                || window_def.draw_callback.eq_ignore_ascii_case("[none]")
            {
                self.apply_default_draw_callback(&mut window_mut);
            }
        }

        {
            let previous_selected = effects.selected_window.replace(Rc::downgrade(&window));
            effects.with_receiving_manager(self, |effects| {
                window.borrow_mut().send_system_message_with_message_sink(
                    WindowMessage::Create,
                    0,
                    0,
                    effects,
                    None,
                );
            });
            effects.selected_window = previous_selected;
        }
        if let Some(parent_window) = parent {
            {
                let previous_selected = effects
                    .selected_window
                    .replace(Rc::downgrade(parent_window));
                effects.with_receiving_manager(self, |effects| {
                    parent_window
                        .borrow_mut()
                        .send_input_message_with_message_sink(
                            WindowMessage::ScriptCreate,
                            window_id as WindowMsgData,
                            0,
                            true,
                            effects,
                            None,
                        );
                });
                effects.selected_window = previous_selected;
            }
        }
        // C++ WinCreateFromScript only pushes top-level WINDOW tokens into
        // scriptInfo.windows (GameWindowManagerScript.cpp:2833-2843).
        // WindowLayout::hide then walks that list only (WindowLayout.cpp:61-64).
        // Adding children here made layout.hide(false) clear authored HIDDEN
        // on Clock / GetUpdate / GetMapPack / StaticTextSelectDifficulty.
        if parent.is_none() {
            layout.borrow_mut().add_window(window.clone());
            info.windows.push(window.clone());
        }
        if window_def.status.contains(WindowStatus::TAB_STOP)
            || (window_def.style | style_for_window_type(&window_def.window_type)) & GWS_TAB_STOP
                != 0
        {
            self.tab_list.push(Rc::downgrade(&window));
        }

        for child_def in &window_def.children {
            self.create_window_from_definition_with_effects(
                child_def,
                Some(&window),
                layout,
                layout_def,
                info,
                &mut *effects,
            )
            .map_err(|err| {
                log::error!(
                    "Failed while creating child '{}' under '{}': {:?}",
                    child_def.name,
                    window_def.name,
                    err
                );
                err
            })?;
        }

        if (window.borrow().get_style() & GWS_TAB_CONTROL) != 0 {
            if !has_tab_pane_child {
                self.create_default_tab_panes(&window, effects)
                    .map_err(|err| {
                        log::error!(
                            "Failed creating default tab panes for '{}': {:?}",
                            window_def.name,
                            err
                        );
                        err
                    })?;
            }
            self.resize_tab_panes(&window, effects);
            let active_index =
                if let Some(WindowWidget::TabControl(tab_control)) = window.borrow().widget() {
                    tab_control.active_tab_index()
                } else {
                    0
                };
            effects.with_receiving_manager(self, |effects| {
                window.borrow_mut().show_tab_pane(active_index, effects)
            });
        }

        if (window.borrow().get_style() & GWS_ALL_SLIDER) != 0 {
            self.create_slider_thumb_child_with_window(
                &window,
                layout_def,
                Some(window_def),
                effects,
            )
            .map_err(|err| {
                log::error!(
                    "Failed creating slider thumb for '{}': {:?}",
                    window_def.name,
                    err
                );
                err
            })?;
        }

        if (window.borrow().get_style() & GWS_COMBO_BOX) != 0 {
            self.create_combo_box_children(&window, layout_def, window_def, effects)
                .map_err(|err| {
                    log::error!(
                        "Failed creating combo-box children for '{}': {:?}",
                        window_def.name,
                        err
                    );
                    err
                })?;
        }

        if (window.borrow().get_style() & GWS_SCROLL_LISTBOX) != 0 {
            if let Some(listbox_data) = window_def.listbox_data.as_ref() {
                if listbox_data.scrollbar {
                    self.create_listbox_scrollbar_children(
                        &window,
                        layout_def,
                        Some(window_def),
                        effects,
                    )
                    .map_err(|err| {
                        log::error!(
                            "Failed creating listbox scrollbar children for '{}': {:?}",
                            window_def.name,
                            err
                        );
                        err
                    })?;
                }
            }
        }

        Ok(window)
    }

    pub(crate) fn resolve_window_rect(
        &self,
        window_def: &WindowDefinition,
        parent: Option<&Rc<RefCell<GameWindow>>>,
    ) -> (i32, i32, i32, i32) {
        if let Some((x1, y1, x2, y2)) = window_def.raw_screen_rect {
            let (screen_w, screen_h) = self.screen_size;
            let (create_w, create_h) = window_def
                .creation_resolution
                .unwrap_or((screen_w.max(1), screen_h.max(1)));
            let x_scale = screen_w as f32 / create_w.max(1) as f32;
            let y_scale = screen_h as f32 / create_h.max(1) as f32;
            // C++ (Int)(real * scale) truncates toward zero.
            let scaled_x1 = (x1 as f32 * x_scale) as i32;
            let scaled_y1 = (y1 as f32 * y_scale) as i32;
            let scaled_x2 = (x2 as f32 * x_scale) as i32;
            let scaled_y2 = (y2 as f32 * y_scale) as i32;
            let (mut rel_x, mut rel_y) = (scaled_x1, scaled_y1);
            if let Some(parent_window) = parent {
                let (parent_x, parent_y) = parent_window.borrow().get_screen_position();
                rel_x -= parent_x;
                rel_y -= parent_y;
            }
            let width = scaled_x2 - scaled_x1;
            let height = scaled_y2 - scaled_y1;
            return (rel_x, rel_y, width, height);
        }

        let (x, y) = window_def.position;
        let (width, height) = window_def.size;
        (x, y, width, height)
    }
}

#[cfg(test)]
#[path = "layout_rollback_tests.rs"]
mod layout_rollback_tests;
