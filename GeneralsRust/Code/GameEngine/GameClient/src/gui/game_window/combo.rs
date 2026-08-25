//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use super::font::Color;
use super::listbox::gadget_list_box_set_colors;
use super::messages::{
    GCM_DEL_ALL, GCM_GET_ITEM_DATA, GCM_GET_SELECTION, GCM_SET_ITEM_DATA, GCM_SET_SELECTION,
    GCM_SET_TEXT, WindowMessage,
};
use super::payload::{WindowMsgData, WindowMsgPayload, pop_payload, push_payload, with_payload};
use super::window_struct::{GameWindow, WindowWidget};
use crate::gui::gadgets::ComboBoxItem;

pub fn gadget_combo_box_get_text(combo_box: &GameWindow) -> String {
    let Some(links) = combo_box.combobox_links else {
        return String::new();
    };
    combo_box
        .find_child_by_id(links.edit_box)
        .and_then(|edit_box| {
            edit_box.borrow().widget().and_then(|widget| match widget {
                WindowWidget::TextEntry(entry) => Some(entry.text().to_string()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

pub fn gadget_combo_box_set_text(combo_box: &mut GameWindow, text: &str) {
    let _ = with_payload(WindowMsgPayload::Text(text.to_string()), |token| {
        combo_box.send_system_message(WindowMessage::User(GCM_SET_TEXT), token, 0)
    });
}

pub fn gadget_combo_box_add_entry(
    combo_box: &mut GameWindow,
    text: &str,
    color: crate::gui::shell::Color,
) -> i32 {
    let index = {
        let Some(WindowWidget::ComboBox(combo)) = combo_box.widget.as_mut() else {
            return -1;
        };
        let index = combo.items().len() as i32;
        combo.add_item(ComboBoxItem::new(index as u32, text));
        index
    };
    if let Some(links) = combo_box.combobox_links {
        if let Some(list_box) = combo_box.find_child_by_id(links.list_box) {
            combo_box.sync_combobox_listbox(&list_box);
            if let Some(WindowWidget::ListBox(listbox)) = list_box.borrow_mut().widget_mut() {
                let _ = listbox.set_item_color(index as usize, color);
            }
            combo_box.resize_combobox_listbox(&list_box);
        }
    }
    index
}

pub fn gadget_combo_box_reset(combo_box: &mut GameWindow) {
    let _ = combo_box.send_system_message(WindowMessage::User(GCM_DEL_ALL), 0, 0);
}

pub fn gadget_combo_box_get_selected_pos(combo_box: &mut GameWindow, selected_index: &mut i32) {
    let token = push_payload(WindowMsgPayload::Int(-1));
    let _ = combo_box.send_system_message(WindowMessage::User(GCM_GET_SELECTION), 0, token);
    if let Some(WindowMsgPayload::Int(value)) = pop_payload(token) {
        *selected_index = value;
    }
}

pub fn gadget_combo_box_set_selected_pos(
    combo_box: &mut GameWindow,
    selected_index: i32,
    dont_hide: bool,
) {
    let _ = combo_box.send_system_message(
        WindowMessage::User(GCM_SET_SELECTION),
        selected_index as WindowMsgData,
        dont_hide as WindowMsgData,
    );
}

pub fn gadget_combo_box_set_item_data(combo_box: &mut GameWindow, index: i32, data: WindowMsgData) {
    let _ = combo_box.send_system_message(
        WindowMessage::User(GCM_SET_ITEM_DATA),
        index as WindowMsgData,
        data,
    );
}

pub fn gadget_combo_box_get_item_data(combo_box: &mut GameWindow, index: i32) -> WindowMsgData {
    let token = push_payload(WindowMsgPayload::UInt(0));
    let _ = combo_box.send_system_message(
        WindowMessage::User(GCM_GET_ITEM_DATA),
        index as WindowMsgData,
        token,
    );
    match pop_payload(token) {
        Some(WindowMsgPayload::UInt(value)) => value,
        Some(WindowMsgPayload::Int(value)) => value as WindowMsgData,
        _ => 0,
    }
}

pub fn gadget_combo_box_get_length(combo_box: &GameWindow) -> i32 {
    match combo_box.widget.as_ref() {
        Some(WindowWidget::ComboBox(combo)) => combo.items().len() as i32,
        _ => 0,
    }
}

pub fn gadget_combo_box_hide_list(combo_box: &mut GameWindow) {
    combo_box.hide_combobox_list();
}

pub fn gadget_combo_box_set_max_display(combo_box: &mut GameWindow, max_display: i32) {
    if let Some(WindowWidget::ComboBox(combo)) = combo_box.widget.as_mut() {
        combo.set_max_display(max_display.max(0) as usize);
    }
}

pub fn gadget_combo_box_set_is_editable(combo_box: &mut GameWindow, editable: bool) {
    combo_box.set_combobox_editable(editable);
}

pub fn gadget_combo_box_set_ascii_only(combo_box: &mut GameWindow, ascii_only: bool) {
    combo_box.set_combobox_validation_flags(Some(ascii_only), None);
}

pub fn gadget_combo_box_set_letters_and_numbers_only(
    combo_box: &mut GameWindow,
    letters_and_numbers_only: bool,
) {
    combo_box.set_combobox_validation_flags(None, Some(letters_and_numbers_only));
}

pub fn gadget_combo_box_set_max_chars(combo_box: &mut GameWindow, max_chars: i32) {
    let max_chars = max_chars.max(0) as usize;
    if let Some(WindowWidget::ComboBox(combo)) = combo_box.widget.as_mut() {
        combo.set_max_chars(max_chars);
    }
    if let Some(links) = combo_box.combobox_links {
        if let Some(edit_box) = combo_box.find_child_by_id(links.edit_box) {
            if let Some(WindowWidget::TextEntry(entry)) = edit_box.borrow_mut().widget_mut() {
                entry.set_max_length(max_chars);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn gadget_combo_box_set_colors(
    combo_box: &mut GameWindow,
    enabled_color: Color,
    enabled_border_color: Color,
    enabled_selected_item_color: Color,
    enabled_selected_item_border_color: Color,
    disabled_color: Color,
    disabled_border_color: Color,
    disabled_selected_item_color: Color,
    disabled_selected_item_border_color: Color,
    hilite_color: Color,
    hilite_border_color: Color,
    hilite_selected_item_color: Color,
    hilite_selected_item_border_color: Color,
) {
    let _ = combo_box.set_enabled_draw_colors(0, enabled_color, enabled_border_color);
    let _ = combo_box.set_enabled_draw_colors(
        1,
        enabled_selected_item_color,
        enabled_selected_item_border_color,
    );
    let _ = combo_box.set_disabled_draw_colors(0, disabled_color, disabled_border_color);
    let _ = combo_box.set_disabled_draw_colors(
        1,
        disabled_selected_item_color,
        disabled_selected_item_border_color,
    );
    let _ = combo_box.set_hilite_draw_colors(0, hilite_color, hilite_border_color);
    let _ = combo_box.set_hilite_draw_colors(
        1,
        hilite_selected_item_color,
        hilite_selected_item_border_color,
    );

    let Some(links) = combo_box.combobox_links else {
        return;
    };

    for child_id in [links.edit_box, links.drop_down] {
        if let Some(child) = combo_box.find_child_by_id(child_id) {
            let mut child = child.borrow_mut();
            let _ = child.set_enabled_draw_colors(0, enabled_color, enabled_border_color);
            let _ = child.set_enabled_draw_colors(
                1,
                enabled_selected_item_color,
                enabled_selected_item_border_color,
            );
            let _ = child.set_disabled_draw_colors(0, disabled_color, disabled_border_color);
            let _ = child.set_disabled_draw_colors(
                1,
                disabled_selected_item_color,
                disabled_selected_item_border_color,
            );
            let _ = child.set_hilite_draw_colors(0, hilite_color, hilite_border_color);
            let _ = child.set_hilite_draw_colors(
                1,
                hilite_selected_item_color,
                hilite_selected_item_border_color,
            );
        }
    }

    if let Some(list_box) = combo_box.find_child_by_id(links.list_box) {
        gadget_list_box_set_colors(
            &mut list_box.borrow_mut(),
            enabled_color,
            enabled_border_color,
            enabled_selected_item_color,
            enabled_selected_item_border_color,
            disabled_color,
            disabled_border_color,
            disabled_selected_item_color,
            disabled_selected_item_border_color,
            hilite_color,
            hilite_border_color,
            hilite_selected_item_color,
            hilite_selected_item_border_color,
        );
    }
}
