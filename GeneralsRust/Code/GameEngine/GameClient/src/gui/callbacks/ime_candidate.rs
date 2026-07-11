//! IMECandidate.cpp callback port.

use crate::gui::display_string::DisplayStringHandle;
use crate::gui::ime_manager::{get_ime_manager, ImeManager};
use crate::gui::{
    get_display_string_manager, get_font_library, with_window_manager, GameWindow,
    WindowInstanceData, WindowMessage, WindowMsgData, WindowMsgHandled, WindowState, WindowStatus,
    WIN_COLOR_UNDEFINED,
};
use std::sync::{Arc, Mutex};

const IME_CANDIDATE_LINE_SPACING: i32 = 2;

thread_local! {
    static DISPLAY_STRING: Arc<Mutex<Option<DisplayStringHandle>>> = Arc::new(Mutex::new(None));
}

fn display_string_slot() -> Arc<Mutex<Option<DisplayStringHandle>>> {
    DISPLAY_STRING.with(|slot| slot.clone())
}

fn ensure_display_string() -> Option<DisplayStringHandle> {
    let slot_handle = display_string_slot();
    let mut slot = slot_handle.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        let mut manager = get_display_string_manager();
        *slot = Some(manager.new_display_string());
    }
    slot.clone()
}

fn free_display_string() {
    let slot_handle = display_string_slot();
    let mut slot = slot_handle.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(handle) = slot.take() {
        let mut manager = get_display_string_manager();
        manager.free_display_string(handle);
    }
}

pub fn ime_candidate_window_input(
    _window: &GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    WindowMsgHandled::Handled
}

pub fn ime_candidate_window_system(
    _window: &GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => {
            ensure_display_string();
            WindowMsgHandled::Handled
        }
        WindowMessage::Destroy => {
            free_display_string();
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn ime_candidate_text_area_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let Some(display_handle) = ensure_display_string() else {
        return;
    };

    let ime_manager = window.get_user_data::<Arc<Mutex<ImeManager>>>().cloned();

    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let text_region = crate::message_stream::game_message::IRegion2D {
        x: origin_x,
        y: origin_y,
        width: size_x,
        height: size_y,
    };

    let enabled = inst_data.status.contains(WindowStatus::ENABLED);
    let hilited = inst_data.state.contains(WindowState::HILITED);

    let (text_color, text_select_color) = if !enabled {
        (inst_data.disabled_text.color, inst_data.disabled_text.color)
    } else if hilited {
        (inst_data.enabled_text.color, inst_data.hilite_text.color)
    } else {
        (inst_data.enabled_text.color, inst_data.hilite_text.color)
    };

    let black = 0xFF000000;
    with_window_manager(|manager| {
        manager.win_open_rect(
            black,
            1.0,
            origin_x,
            origin_y,
            origin_x + size_x,
            origin_y + size_y,
        );
    });

    let font_desc = window.get_font().map(|font| font.to_font_desc());
    let font = font_desc.and_then(|desc| get_font_library().get_font(&desc).ok());
    let Some(font) = font else {
        return;
    };

    let mut display = display_handle.borrow_mut();
    display.set_font(font.clone());
    display.set_clip_region(Some(text_region));

    let line_height = font.height + IME_CANDIDATE_LINE_SPACING;

    let manager = ime_manager.unwrap_or_else(get_ime_manager);
    let ime_guard = match manager.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let first = ime_guard.candidate_page_start();
    let total = ime_guard.candidate_count();
    let page_size = ime_guard.candidate_page_size();
    let selected_index = ime_guard.selected_candidate_index();
    let index_base = ime_guard.index_base();

    if total == 0 || page_size == 0 || first >= total {
        return;
    }

    let mut count = page_size;
    if first + count > total {
        count = total - first;
    }

    let selected = selected_index.saturating_sub(first);

    display.set_text("00:");
    let width = display.get_width(-1);
    let mut y = origin_y;
    let left_edge = origin_x + 10 + width;

    for i in 0..count {
        let Some(candidate) = ime_guard.candidate(first + i) else {
            continue;
        };

        let tcolor = if i == selected {
            text_select_color
        } else {
            text_color
        };

        let number_text = format!("{}:", i as i32 + index_base);
        display.set_text(number_text);
        let number_width = display.get_width(-1);
        display.draw(left_edge - number_width, y, tcolor, black);

        display.set_text(candidate);
        display.draw(left_edge, y, tcolor, black);
        y += line_height;
    }
}

pub fn ime_candidate_main_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (origin_x, origin_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();

    let enabled = inst_data.status.contains(WindowStatus::ENABLED);
    let hilited = inst_data.state.contains(WindowState::HILITED);

    let (back_color, back_border) = if !enabled {
        (
            inst_data.disabled_draw_data[0].color,
            inst_data.disabled_draw_data[0].border_color,
        )
    } else if hilited {
        (
            inst_data.hilite_draw_data[0].color,
            inst_data.hilite_draw_data[0].border_color,
        )
    } else {
        (
            inst_data.enabled_draw_data[0].color,
            inst_data.enabled_draw_data[0].border_color,
        )
    };

    if back_border != WIN_COLOR_UNDEFINED {
        with_window_manager(|manager| {
            manager.win_open_rect(
                back_border,
                1.0,
                origin_x,
                origin_y,
                origin_x + size_x,
                origin_y + size_y,
            );
        });
    }

    if back_color != WIN_COLOR_UNDEFINED {
        with_window_manager(|manager| {
            manager.win_fill_rect(
                back_color,
                0.0,
                origin_x + 1,
                origin_y + 1,
                origin_x + size_x - 1,
                origin_y + size_y - 1,
            );
        });
    }
}
