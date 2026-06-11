//! DownloadMenu.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::gui::callbacks::message_box::message_box_ok;
use crate::gui::{
    with_window_manager, write_input_focus_response, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::language::Language;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::download_manager::{
    download_manager, set_download_manager, DownloadEvent, DownloadManager, DownloadProgress,
    QueuedDownload,
};
use gamelogic::helpers::TheGameLogic;
use gamelogic::helpers::TheGameText;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
struct DownloadMenuState {
    button_cancel_id: i32,
    static_text_size_id: i32,
    static_text_time_id: i32,
    static_text_file_id: i32,
    static_text_status_id: i32,
    progress_bar_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    static_text_size: Option<Rc<RefCell<GameWindow>>>,
    static_text_time: Option<Rc<RefCell<GameWindow>>>,
    static_text_file: Option<Rc<RefCell<GameWindow>>>,
    static_text_status: Option<Rc<RefCell<GameWindow>>>,
    progress_bar: Option<Rc<RefCell<GameWindow>>>,
    last_update: Option<Instant>,
    time_left: i64,
}

thread_local! {
    static DOWNLOAD_MENU_STATE: Arc<Mutex<DownloadMenuState>> =
        Arc::new(Mutex::new(DownloadMenuState::default()));
}

fn download_menu_state() -> Arc<Mutex<DownloadMenuState>> {
    DOWNLOAD_MENU_STATE.with(|state| state.clone())
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn close_download_window() {
    let state_handle = download_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    let Some(parent) = state.parent.take() else {
        return;
    };
    if let Some(layout) = parent.borrow().get_layout() {
        layout.borrow().run_shutdown(None);
        layout.borrow_mut().destroy_windows();
    }
    state.static_text_size = None;
    state.static_text_time = None;
    state.static_text_file = None;
    state.static_text_status = None;
    state.progress_bar = None;
    state.last_update = None;
    state.time_left = 0;

    with_window_manager(|manager| {
        let main_win = manager
            .get_window_by_id(NameKeyGenerator::name_to_key("MainMenu.wnd:MainMenuParent") as i32);
        if let Some(main_win) = main_win {
            let _ = manager.set_focus(Some(&main_win));
        }
    });
}

fn update_progress_bar(window: &Option<Rc<RefCell<GameWindow>>>, progress: &DownloadProgress) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(widget) = guard.widget_mut() {
        if let crate::gui::game_window::WindowWidget::ProgressBar(bar) = widget {
            if progress.total_size > 0 {
                let percent = (progress.bytes_read as f32) * 100.0 / (progress.total_size as f32);
                bar.set_percentage(percent);
            } else {
                bar.set_percentage(0.0);
            }
        }
    }
}

fn update_static_text(window: &Option<Rc<RefCell<GameWindow>>>, text: &str) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let _ = window.borrow_mut().set_text(text);
}

fn handle_download_error() {
    let error_key = {
        let mut guard = download_manager().lock().unwrap_or_else(|e| e.into_inner());
        guard
            .as_mut()
            .map(|manager| manager.error_key().to_string())
            .unwrap_or_else(|| "FTP:UnknownError".to_string())
    };
    let title = TheGameText::fetch("GUI:DownloadErrorTitle");
    let body = TheGameText::fetch(&error_key);
    message_box_ok(
        &title,
        &body,
        Some(Box::new(|| {
            crate::gui::shell::main_menu::get_main_menu().handle_canceled_download(true);
            close_download_window();
        })),
    );
}

fn handle_download_success(should_quit: bool) {
    let title = TheGameText::fetch("GUI:DownloadSuccessTitle");
    let body_key = if should_quit {
        "GUI:DownloadSuccessMustQuit"
    } else {
        "GUI:DownloadSuccess"
    };
    let body = TheGameText::fetch(body_key);
    message_box_ok(
        &title,
        &body,
        Some(Box::new(move || {
            if should_quit {
                if let Some(engine) = get_game_engine() {
                    let mut engine = engine.lock();
                    engine.set_quitting(true);
                }
                if TheGameLogic::is_in_game() {
                    let _ = TheGameLogic::clear_game_data();
                }
            } else {
                crate::gui::shell::main_menu::get_main_menu().handle_canceled_download(true);
            }
            close_download_window();
        })),
    );
}

fn update_time_text(state: &mut DownloadMenuState) {
    let Some(last_update) = state.last_update else {
        return;
    };
    if last_update.elapsed() < Duration::from_secs(1) {
        return;
    }
    state.last_update = Some(Instant::now());
    if let Some(window) = state.static_text_time.as_ref() {
        let time_text = format_time_left(state.time_left);
        let _ = window.borrow_mut().set_text(&time_text);
    }
}

fn format_time_left(time_left: i64) -> String {
    if time_left > 0 {
        let taken_hour = time_left / 3600;
        let taken_min = time_left / 60;
        let taken_sec = time_left % 60;
        Language::format_localized_string(
            "GUI:DownloadTimeLeft",
            &[
                &taken_hour.to_string(),
                &taken_min.to_string(),
                &taken_sec.to_string(),
            ],
        )
    } else {
        TheGameText::fetch("GUI:DownloadUnknownTime")
    }
}

fn update_from_event(state: &mut DownloadMenuState, event: DownloadEvent) {
    match event {
        DownloadEvent::FileStarted(file) => {
            let file_name = file.rsplit(['/', '\\']).next().unwrap_or(file.as_str());
            update_static_text(&state.static_text_file, file_name);
        }
        DownloadEvent::StatusUpdate(_) => {
            let status_key = {
                let mut guard = download_manager().lock().unwrap_or_else(|e| e.into_inner());
                guard
                    .as_mut()
                    .map(|manager| manager.status_key().to_string())
                    .unwrap_or_else(|| "FTP:StatusNone".to_string())
            };
            let status = TheGameText::fetch(&status_key);
            update_static_text(&state.static_text_status, &status);
        }
        DownloadEvent::Progress(progress) => {
            update_progress_bar(&state.progress_bar, &progress);
            let size_text = Language::format_localized_string(
                "GUI:DownloadBytesRatio",
                &[
                    &progress.bytes_read.to_string(),
                    &progress.total_size.to_string(),
                ],
            );
            update_static_text(&state.static_text_size, &size_text);
            state.time_left = progress.time_left;
            if let Some(time_window) = state.static_text_time.as_ref() {
                if time_window.borrow().get_text().is_empty() {
                    let time_text = format_time_left(state.time_left);
                    let _ = time_window.borrow_mut().set_text(&time_text);
                    state.last_update = Some(Instant::now());
                }
            }
        }
        DownloadEvent::Error(_) => {
            handle_download_error();
        }
        DownloadEvent::End => {
            let (should_start_next, should_quit) = {
                let mut guard = download_manager().lock().unwrap_or_else(|e| e.into_inner());
                guard
                    .as_mut()
                    .map(|manager| {
                        let has_more = manager.is_file_queued_for_download();
                        let local = manager.last_local_file().to_string();
                        (has_more, local.contains("patches"))
                    })
                    .unwrap_or((false, false))
            };
            if should_start_next {
                start_next_download();
            } else {
                handle_download_success(should_quit);
            }
        }
    }
}

pub fn download_menu_init(_layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let state_handle = download_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.button_cancel_id = name_to_id("DownloadMenu.wnd:ButtonCancel");
    state.static_text_size_id = name_to_id("DownloadMenu.wnd:StaticTextSize");
    state.static_text_time_id = name_to_id("DownloadMenu.wnd:StaticTextTime");
    state.static_text_file_id = name_to_id("DownloadMenu.wnd:StaticTextFile");
    state.static_text_status_id = name_to_id("DownloadMenu.wnd:StaticTextStatus");
    state.progress_bar_id = name_to_id("DownloadMenu.wnd:ProgressBarMunkee");

    state.parent = with_window_manager(|manager| {
        manager.get_window_by_id(
            NameKeyGenerator::name_to_key("DownloadMenu.wnd:ParentDownload") as i32,
        )
    });

    let parent = state.parent.clone();
    if let Some(parent) = parent {
        let parent_guard = parent.borrow();
        state.static_text_size = parent_guard.find_child_by_id(state.static_text_size_id);
        state.static_text_time = parent_guard.find_child_by_id(state.static_text_time_id);
        state.static_text_file = parent_guard.find_child_by_id(state.static_text_file_id);
        state.static_text_status = parent_guard.find_child_by_id(state.static_text_status_id);
        state.progress_bar = parent_guard.find_child_by_id(state.progress_bar_id);
    }

    let mut guard = download_manager().lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(DownloadManager::new());
    }
}

pub fn download_menu_shutdown(_layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    set_download_manager(None);
    let state_handle = download_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.parent = None;
    state.static_text_size = None;
    state.static_text_time = None;
    state.static_text_file = None;
    state.static_text_status = None;
    state.progress_bar = None;
    state.last_update = None;
    state.time_left = 0;
}

pub fn download_menu_update(_layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let state_handle = download_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    let events = {
        let mut guard = download_manager().lock().unwrap_or_else(|e| e.into_inner());
        guard
            .as_mut()
            .map(|manager| manager.update())
            .unwrap_or_default()
    };
    for event in events {
        update_from_event(&mut state, event);
    }
    update_time_text(&mut state);
}

pub fn download_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char {
        return WindowMsgHandled::Ignored;
    }
    if data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    crate::gui::shell::main_menu::get_main_menu().handle_canceled_download(true);
    close_download_window();
    WindowMsgHandled::Handled
}

pub fn download_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let state_handle = download_menu_state();
            if control_id
                == state_handle
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .button_cancel_id
            {
                crate::gui::shell::main_menu::get_main_menu().handle_canceled_download(true);
                close_download_window();
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_menu_system_consumes_lifecycle_messages_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            download_menu_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            download_menu_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            download_menu_system(&window, WindowMessage::InputFocus, 1, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            download_menu_system(&window, WindowMessage::MousePos, 0, 0),
            WindowMsgHandled::Ignored
        );
    }
}

pub fn queue_download(download: QueuedDownload) {
    let mut guard = download_manager().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(manager) = guard.as_mut() {
        manager.queue_file_for_download(download);
    }
}

pub fn start_next_download() {
    let mut guard = download_manager().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(manager) = guard.as_mut() {
        let _ = manager.download_next_queued_file();
    }
}
