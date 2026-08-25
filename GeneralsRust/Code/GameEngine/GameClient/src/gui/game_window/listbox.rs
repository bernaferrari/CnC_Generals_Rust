//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use super::font::Color;
use super::messages::{GLM_GET_SELECTION, WindowMessage};
use super::payload::{WindowMsgData, WindowMsgPayload, pop_payload, push_payload};
use super::window_struct::{GameWindow, WindowWidget};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListBoxSelectionResult {
    pub single: i32,
    pub multiple: Vec<i32>,
}

pub fn gadget_list_box_get_selected(listbox: &mut GameWindow, select_list: &mut WindowMsgData) {
    let token = push_payload(WindowMsgPayload::None);
    let _ = listbox.send_system_message(WindowMessage::User(GLM_GET_SELECTION), 0, token);
    match pop_payload(token) {
        Some(WindowMsgPayload::Int(index)) => {
            *select_list = index as isize as WindowMsgData;
        }
        Some(payload @ WindowMsgPayload::IntList(_)) => {
            *select_list = push_payload(payload);
        }
        _ => *select_list = 0,
    }
}

pub fn gadget_list_box_get_bottom_visible_entry(listbox: &GameWindow) -> i32 {
    match listbox.widget.as_ref() {
        Some(WindowWidget::ListBox(listbox)) => listbox.get_bottom_visible_entry(),
        _ => 0,
    }
}

pub fn gadget_list_box_is_full(listbox: &GameWindow) -> bool {
    match listbox.widget.as_ref() {
        Some(WindowWidget::ListBox(listbox)) => listbox.is_full(),
        _ => false,
    }
}

pub fn gadget_list_box_set_bottom_visible_entry(listbox: &mut GameWindow, index: i32) {
    if let Some(WindowWidget::ListBox(listbox)) = listbox.widget.as_mut() {
        listbox.set_bottom_visible_entry(index);
    }
}

pub fn gadget_list_box_get_top_visible_entry(listbox: &GameWindow) -> i32 {
    match listbox.widget.as_ref() {
        Some(WindowWidget::ListBox(listbox)) => listbox.get_top_visible_entry(),
        _ => 0,
    }
}

pub fn gadget_list_box_set_top_visible_entry(listbox: &mut GameWindow, index: i32) {
    if let Some(WindowWidget::ListBox(listbox)) = listbox.widget.as_mut() {
        listbox.set_top_visible_entry(index);
    }
}

pub fn gadget_list_box_set_audio_feedback(listbox: &mut GameWindow, enable: bool) {
    if let Some(WindowWidget::ListBox(listbox)) = listbox.widget.as_mut() {
        listbox.set_audio_feedback(enable);
    }
}

pub fn gadget_list_box_get_num_columns(listbox: &GameWindow) -> i32 {
    match listbox.widget.as_ref() {
        Some(WindowWidget::ListBox(listbox)) => listbox.columns() as i32,
        _ => 0,
    }
}

pub fn gadget_list_box_get_column_width(listbox: &GameWindow, column: i32) -> i32 {
    match listbox.widget.as_ref() {
        Some(WindowWidget::ListBox(listbox)) => listbox.column_width(column) as i32,
        _ => 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn gadget_list_box_set_colors(
    listbox: &mut GameWindow,
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
    let _ = listbox.set_enabled_draw_colors(0, enabled_color, enabled_border_color);
    let _ = listbox.set_enabled_draw_colors(
        1,
        enabled_selected_item_color,
        enabled_selected_item_border_color,
    );
    let _ = listbox.set_disabled_draw_colors(0, disabled_color, disabled_border_color);
    let _ = listbox.set_disabled_draw_colors(
        1,
        disabled_selected_item_color,
        disabled_selected_item_border_color,
    );
    let _ = listbox.set_hilite_draw_colors(0, hilite_color, hilite_border_color);
    let _ = listbox.set_hilite_draw_colors(
        1,
        hilite_selected_item_color,
        hilite_selected_item_border_color,
    );

    let Some(links) = listbox.listbox_links else {
        return;
    };
    let Some(slider) = listbox.find_child_by_id(links.slider) else {
        return;
    };
    {
        let mut slider = slider.borrow_mut();
        let _ = slider.set_enabled_draw_colors(0, enabled_color, enabled_border_color);
        let _ = slider.set_disabled_draw_colors(0, disabled_color, disabled_border_color);
        let _ = slider.set_hilite_draw_colors(0, hilite_color, hilite_border_color);
    }

    let thumb_selected = links
        .thumb
        .and_then(|thumb_id| listbox.find_child_by_id(thumb_id));
    let enabled_selected = thumb_selected
        .as_ref()
        .and_then(|thumb| thumb.borrow().get_enabled_draw_data(1))
        .unwrap_or_default();
    let disabled_selected = thumb_selected
        .as_ref()
        .and_then(|thumb| thumb.borrow().get_disabled_draw_data(1))
        .unwrap_or_default();
    let hilite_selected = thumb_selected
        .as_ref()
        .and_then(|thumb| thumb.borrow().get_hilite_draw_data(1))
        .unwrap_or_default();

    for button_id in [links.up_button, links.down_button] {
        if let Some(button) = listbox.find_child_by_id(button_id) {
            let mut button = button.borrow_mut();
            let _ = button.set_enabled_draw_colors(0, enabled_color, enabled_border_color);
            let _ = button.set_enabled_draw_colors(
                1,
                enabled_selected.color,
                enabled_selected.border_color,
            );
            let _ = button.set_disabled_draw_colors(0, disabled_color, disabled_border_color);
            let _ = button.set_disabled_draw_colors(
                1,
                disabled_selected.color,
                disabled_selected.border_color,
            );
            let _ = button.set_hilite_draw_colors(0, hilite_color, hilite_border_color);
            let _ = button.set_hilite_draw_colors(
                1,
                hilite_selected.color,
                hilite_selected.border_color,
            );
        }
    }
}
