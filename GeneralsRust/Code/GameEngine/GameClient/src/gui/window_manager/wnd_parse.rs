//! .wnd parse helpers: path resolve, style/widget mapping, text/tooltip apply.
#![allow(unused_imports)]

use crate::game_text::GameText;
use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    RadioButtonGroup, StaticText, TabControl, TextEntry, VerticalSlider,
};
use crate::gui::game_window::*;
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

/// C++ WND `"[None]"` / empty string: FunctionLexicon lookup fails → NULL callback.
pub(crate) fn is_none_callback_name(name: &str) -> bool {
    let name = name.trim();
    name.is_empty() || name.eq_ignore_ascii_case("[none]")
}

pub(crate) fn resolve_window_script_path(filename: &str) -> WindowResult<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        for base in current_dir.ancestors() {
            candidates.push(
                base.join("windows_game/extracted_big_files_v2/WindowZH/Window")
                    .join(filename),
            );
            candidates.push(
                base.join("windows_game/extracted_big_files_v2/WindowZH/Window/Menus")
                    .join(filename),
            );
            candidates.push(
                base.join("windows_game/extracted_big_files/WindowZH/Window")
                    .join(filename),
            );
            candidates.push(
                base.join("windows_game/extracted_big_files/WindowZH/Window/Menus")
                    .join(filename),
            );
            if let Some(bare) = filename.rsplit(['/', '\\']).next() {
                if bare != filename {
                    candidates.push(
                        base.join("windows_game/extracted_big_files_v2/WindowZH/Window/Menus")
                            .join(bare),
                    );
                    candidates.push(
                        base.join("windows_game/extracted_big_files/WindowZH/Window/Menus")
                            .join(bare),
                    );
                }
            }
        }
    }
    candidates
        .push(Path::new("windows_game/extracted_big_files_v2/WindowZH/Window").join(filename));
    candidates.push(
        Path::new("windows_game/extracted_big_files_v2/WindowZH/Window/Menus").join(filename),
    );
    candidates.push(Path::new("windows_game/extracted_big_files/WindowZH/Window").join(filename));
    candidates
        .push(Path::new("windows_game/extracted_big_files/WindowZH/Window/Menus").join(filename));
    candidates.push(Path::new(filename).to_path_buf());
    for path in candidates {
        if path.exists() {
            return Ok(path);
        }
    }
    Err(WindowError::InvalidParameter)
}

pub(crate) fn style_for_window_type(window_type: &str) -> u32 {
    match window_type.trim().to_ascii_uppercase().as_str() {
        "PUSHBUTTON" => GWS_PUSH_BUTTON,
        "RADIOBUTTON" => GWS_RADIO_BUTTON,
        "CHECKBOX" => GWS_CHECK_BOX,
        "VERTSLIDER" => GWS_VERT_SLIDER,
        "HORZSLIDER" => GWS_HORZ_SLIDER,
        "SCROLLLISTBOX" => GWS_SCROLL_LISTBOX,
        "ENTRYFIELD" => GWS_ENTRY_FIELD,
        "STATICTEXT" => GWS_STATIC_TEXT,
        "PROGRESSBAR" => GWS_PROGRESS_BAR,
        "USER" => GWS_USER_WINDOW,
        "TABCONTROL" => GWS_TAB_CONTROL,
        "TABPANE" => GWS_TAB_PANE,
        "COMBOBOX" => GWS_COMBO_BOX,
        _ => 0,
    }
}

pub(crate) fn create_widget_for_style(
    radio_groups: &mut HashMap<u32, RadioButtonGroup>,
    window_def: &WindowDefinition,
    window_id: WindowId,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Option<WindowWidget> {
    let gadget_id = if window_id > 0 { window_id as u32 } else { 0 };
    let width_u = width.max(0) as u32;
    let height_u = height.max(0) as u32;
    let size = width.min(height).max(0) as u32;
    let text = if !window_def.text.is_empty() {
        window_def.text.clone()
    } else {
        window_def.text_label.clone()
    };

    let style = window_def.style | style_for_window_type(&window_def.window_type);
    if style & GWS_PUSH_BUTTON != 0 {
        let mut button = PushButton::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            button.set_text(text);
        }
        return Some(WindowWidget::PushButton(button));
    }
    if style & GWS_RADIO_BUTTON != 0 {
        let group_id = window_def
            .radio_button_data
            .as_ref()
            .map(|data| data.group)
            .unwrap_or(gadget_id);
        let group = radio_groups
            .entry(group_id)
            .or_insert_with(|| RadioButtonGroup::new(group_id))
            .clone();
        let mut radio = RadioButton::new(gadget_id, x, y, size, group);
        if !text.is_empty() {
            radio.set_label(text);
        }
        return Some(WindowWidget::RadioButton(radio));
    }
    if style & GWS_CHECK_BOX != 0 {
        let checkbox = crate::gui::gadgets::CheckBox::new(gadget_id, x, y, size);
        return Some(WindowWidget::CheckBox(checkbox));
    }
    if style & GWS_VERT_SLIDER != 0 {
        return Some(WindowWidget::VerticalSlider(VerticalSlider::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_HORZ_SLIDER != 0 {
        return Some(WindowWidget::HorizontalSlider(HorizontalSlider::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_SCROLL_LISTBOX != 0 {
        return Some(WindowWidget::ListBox(ListBox::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_ENTRY_FIELD != 0 {
        let mut entry = TextEntry::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            entry.set_text(text);
        }
        return Some(WindowWidget::TextEntry(entry));
    }
    if style & GWS_STATIC_TEXT != 0 {
        let mut label = StaticText::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            label.set_text(text);
        }
        return Some(WindowWidget::StaticText(label));
    }
    if style & GWS_PROGRESS_BAR != 0 {
        return Some(WindowWidget::ProgressBar(ProgressBar::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_USER_WINDOW != 0 {
        return Some(WindowWidget::User);
    }
    if style & GWS_MOUSE_TRACK != 0 {
        return Some(WindowWidget::MouseTrack);
    }
    if style & GWS_ANIMATED != 0 {
        return Some(WindowWidget::Animated);
    }
    if style & GWS_TAB_CONTROL != 0 {
        return Some(WindowWidget::TabControl(TabControl::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_TAB_PANE != 0 {
        return Some(WindowWidget::TabPane);
    }
    if style & GWS_COMBO_BOX != 0 {
        return Some(WindowWidget::ComboBox(ComboBox::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }

    None
}

pub(crate) fn apply_window_text(window: &mut GameWindow, window_def: &WindowDefinition) {
    let text = if !window_def.text_label.is_empty() {
        GameText::fetch(&window_def.text_label)
    } else if !window_def.text.is_empty() {
        if window_def.text.contains(':') && !window_def.text.contains(' ') {
            GameText::fetch(&window_def.text)
        } else {
            window_def.text.clone()
        }
    } else {
        return;
    };

    let _ = window.set_text(&text);
}

pub(crate) fn apply_window_tooltip(window: &mut GameWindow, window_def: &WindowDefinition) {
    if window_def.tooltip.is_empty() {
        return;
    }
    let tooltip = GameText::fetch(&window_def.tooltip);
    window.set_tooltip(&tooltip);
    if let Some(widget) = window.widget_mut() {
        if let WindowWidget::ListBox(listbox) = widget {
            listbox.set_tooltip(tooltip);
        }
    }
}

pub(crate) fn map_window_message_to_main_menu(msg: WindowMessage) -> u32 {
    const GGM_LEFT_DRAG: u32 = 16384;
    const GBM_MOUSE_ENTERING: u32 = GGM_LEFT_DRAG + 6;
    const GBM_MOUSE_LEAVING: u32 = GGM_LEFT_DRAG + 7;
    const GBM_SELECTED: u32 = GGM_LEFT_DRAG + 8;
    const GBM_SELECTED_RIGHT: u32 = GGM_LEFT_DRAG + 9;

    match msg {
        WindowMessage::Create => 1,
        WindowMessage::Destroy => 2,
        WindowMessage::Char => 21,
        WindowMessage::InputFocus => 23,
        WindowMessage::MousePos => 24,
        WindowMessage::GadgetMouseEntering => GBM_MOUSE_ENTERING,
        WindowMessage::GadgetMouseLeaving => GBM_MOUSE_LEAVING,
        WindowMessage::GadgetSelected => GBM_SELECTED,
        WindowMessage::GadgetRightClick => GBM_SELECTED_RIGHT,
        _ => 0,
    }
}

pub(crate) fn apply_window_status_to_widget(window: &mut GameWindow) {
    let status = window.get_status();
    if let Some(widget) = window.widget_mut() {
        if let WindowWidget::PushButton(button) = widget {
            if status.contains(WindowStatus::CHECK_LIKE) {
                button.set_checkbox(true, false);
            }
            if status.contains(WindowStatus::ON_MOUSE_DOWN) {
                button.set_triggers_on_mouse_down(true);
            }
        }
    }
}

pub(crate) fn apply_window_widget_data(window: &mut GameWindow, window_def: &WindowDefinition) {
    if let Some(widget) = window.widget_mut() {
        match widget {
            WindowWidget::ListBox(listbox) => {
                if let Some(data) = window_def.listbox_data.as_ref() {
                    if data.length > 0 {
                        listbox.set_max_length(data.length);
                    }
                    listbox.set_auto_purge(data.autopurge);
                    listbox.set_auto_scroll(data.autoscroll);
                    listbox.set_scroll_if_at_end(data.scroll_if_at_end);
                    listbox.set_force_select(data.force_select);
                    listbox.set_columns(data.columns);
                    if !data.column_widths.is_empty() {
                        listbox.set_column_width_percentages(data.column_widths.clone());
                    }
                    if data.multiselect {
                        listbox.set_selection_mode(crate::gui::gadgets::SelectionMode::Multiple);
                    }
                }
            }
            WindowWidget::TextEntry(entry) => {
                if let Some(data) = window_def.text_entry_data.as_ref() {
                    if data.max_len > 0 {
                        entry.set_max_length(data.max_len);
                    }
                    entry.set_password(data.secret_text);
                    let validation = if data.numerical_only {
                        crate::gui::gadgets::ValidationMode::NumericOnly
                    } else if data.alphanumerical_only {
                        crate::gui::gadgets::ValidationMode::AlphanumericOnly
                    } else if data.ascii_only {
                        crate::gui::gadgets::ValidationMode::AsciiOnly
                    } else {
                        crate::gui::gadgets::ValidationMode::None
                    };
                    entry.set_validation(validation);
                }
            }
            WindowWidget::StaticText(label) => {
                if let Some(data) = window_def.static_text_data.as_ref() {
                    let horizontal = if data.centered {
                        crate::gui::gadgets::TextAlignment::Center
                    } else {
                        crate::gui::gadgets::TextAlignment::Left
                    };
                    let vertical = if data.centered_vertically {
                        crate::gui::gadgets::VerticalAlignment::Center
                    } else {
                        crate::gui::gadgets::VerticalAlignment::Top
                    };
                    label.set_alignment(horizontal, vertical);
                    label.set_margins(data.left_margin, data.top_margin);
                }
            }
            WindowWidget::HorizontalSlider(slider) => {
                if let Some(data) = window_def.slider_data.as_ref() {
                    slider.set_range(data.min_value, data.max_value);
                    window.update_slider_thumb();
                }
            }
            WindowWidget::VerticalSlider(slider) => {
                if let Some(data) = window_def.slider_data.as_ref() {
                    slider.set_range(data.min_value, data.max_value);
                    window.update_slider_thumb();
                }
            }
            WindowWidget::ComboBox(combo) => {
                if let Some(data) = window_def.combo_box_data.as_ref() {
                    combo.set_editable(data.is_editable);
                    if data.max_chars > 0 {
                        combo.set_max_chars(data.max_chars);
                    }
                    combo.set_ascii_only(data.ascii_only);
                    combo.set_letters_and_numbers(data.letters_and_numbers);
                    if data.max_display > 0 {
                        combo.set_max_display(data.max_display);
                    }
                }
            }
            WindowWidget::TabControl(tab_control) => {
                if let Some(data) = window_def.tab_control_data.as_ref() {
                    tab_control.set_tab_data(crate::gui::gadgets::TabControlData {
                        tab_orientation: data.tab_orientation,
                        tab_edge: data.tab_edge,
                        tab_width: data.tab_width,
                        tab_height: data.tab_height,
                        tab_count: data.tab_count,
                        pane_border: data.pane_border,
                        sub_pane_disabled: data.sub_pane_disabled,
                    });
                }
            }
            _ => {}
        }
    }
}
