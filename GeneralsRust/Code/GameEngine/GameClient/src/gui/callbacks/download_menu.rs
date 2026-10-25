//! DownloadMenu.cpp callback port.

use crate::gui::game_window::WindowEventEffects;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::gui::callbacks::message_box::message_box_ok;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
    write_input_focus_response,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::language::Language;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::download_manager::{
    DownloadEvent, DownloadManager, DownloadProgress, QueuedDownload, download_manager,
    set_download_manager,
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

// THREAD: the download screen only exists on the GUI thread (C++ kept this
// state in the dialog itself), so the menu state is a plain thread-local
// `RefCell` instead of a locked slot — a nested borrow now panics instead of
// wedging a non-re-entrant mutex.
thread_local! {
    static DOWNLOAD_MENU_STATE: Rc<RefCell<DownloadMenuState>> =
        Rc::new(RefCell::new(DownloadMenuState::default()));
}

fn download_menu_state() -> Rc<RefCell<DownloadMenuState>> {
    DOWNLOAD_MENU_STATE.with(|state| state.clone())
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn close_download_window(
    manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) {
    let parent = {
        let state_handle = download_menu_state();
        let mut state = state_handle.borrow_mut();
        let Some(parent) = state.parent.take() else {
            return;
        };
        state.static_text_size = None;
        state.static_text_time = None;
        state.static_text_file = None;
        state.static_text_status = None;
        state.progress_bar = None;
        state.last_update = None;
        state.time_left = 0;
        parent
    };
    // Shutdown can re-enter this menu. Its current-window identity and state
    // loan have already been resolved, so recursion observes the completed take.
    let layout = if let Some(window) = selected
        .as_deref()
        .filter(|window| std::ptr::eq(parent.as_ptr(), std::ptr::from_ref(*window)))
    {
        window.get_layout()
    } else {
        parent.borrow().get_layout()
    };
    if let Some(layout) = layout {
        effects.with_receiving_manager(manager, |effects| {
            layout.borrow().run_shutdown_with_effects(None, effects)
        });
        layout.borrow_mut().destroy_windows(manager, effects);
    }
    let id = NameKeyGenerator::name_to_key("MainMenu.wnd:MainMenuParent") as i32;
    if let Some(main_win) = manager.get_window_by_id_with_selected(id, selected.as_deref()) {
        let _ = manager.set_focus_with_selected(Some(&main_win), effects, selected.as_deref_mut());
    }
}

/// A message-box completion has no direct loan of its mutable parent. Finish
/// this bounded UI cleanup after that callback releases the window, before the
/// next input is admitted; never capture its originating manager in the dialog.
fn queue_download_finish(reset_dropdown: bool, effects: &mut WindowEventEffects<'_>) {
    effects.queue_window_operation(move |manager, effects| {
        if reset_dropdown {
            effects.with_receiving_manager(manager, |effects| {
                effects.with_main_menu(|menu, effects| {
                    menu.handle_canceled_download(true, effects, None)
                })
            });
        }
        close_download_window(manager, effects, None);
    });
}

fn cancel_download(window: &mut GameWindow, effects: &mut WindowEventEffects<'_>) {
    effects.with_main_menu(|menu, effects| {
        menu.handle_canceled_download(true, effects, Some(&mut *window))
    });
    effects.with_manager(|manager, effects| {
        close_download_window(manager, effects, Some(&mut *window))
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

fn handle_download_error(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let error_key = {
        let download_slot = download_manager();
        let mut guard = download_slot
            .lock()
            .expect("download manager slot re-entered on the GUI thread");
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
        Some(Box::new(|effects| {
            queue_download_finish(true, effects);
        })),
        receiving_manager,
        effects,
    );
}

fn handle_download_success(
    should_quit: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
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
        Some(Box::new(move |effects| {
            if should_quit {
                if let Some(engine) = get_game_engine() {
                    let mut engine = engine.lock();
                    engine.set_quitting(true);
                }
                if TheGameLogic::is_in_game() {
                    let _ = TheGameLogic::clear_game_data();
                }
            }
            queue_download_finish(!should_quit, effects);
        })),
        receiving_manager,
        effects,
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

fn update_from_event(
    state: &mut DownloadMenuState,
    event: DownloadEvent,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    match event {
        DownloadEvent::FileStarted(file) => {
            let file_name = file.rsplit(['/', '\\']).next().unwrap_or(file.as_str());
            update_static_text(&state.static_text_file, file_name);
        }
        DownloadEvent::StatusUpdate(_) => {
            let status_key = {
                let download_slot = download_manager();
                let mut guard = download_slot
                    .lock()
                    .expect("download manager slot re-entered on the GUI thread");
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
            handle_download_error(receiving_manager, effects);
        }
        DownloadEvent::End => {
            let (should_start_next, should_quit) = {
                let download_slot = download_manager();
                let mut guard = download_slot
                    .lock()
                    .expect("download manager slot re-entered on the GUI thread");
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
                handle_download_success(should_quit, receiving_manager, effects);
            }
        }
    }
}

pub fn download_menu_init(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    let state_handle = download_menu_state();
    let mut state = state_handle.borrow_mut();

    state.button_cancel_id = name_to_id("DownloadMenu.wnd:ButtonCancel");
    state.static_text_size_id = name_to_id("DownloadMenu.wnd:StaticTextSize");
    state.static_text_time_id = name_to_id("DownloadMenu.wnd:StaticTextTime");
    state.static_text_file_id = name_to_id("DownloadMenu.wnd:StaticTextFile");
    state.static_text_status_id = name_to_id("DownloadMenu.wnd:StaticTextStatus");
    state.progress_bar_id = name_to_id("DownloadMenu.wnd:ProgressBarMunkee");

    state.parent = (|manager: &mut crate::gui::window_manager::WindowManager| {
        manager.get_window_by_id(
            NameKeyGenerator::name_to_key("DownloadMenu.wnd:ParentDownload") as i32,
        )
    })(receiving_manager);

    let parent = state.parent.clone();
    if let Some(parent) = parent {
        let parent_guard = parent.borrow();
        state.static_text_size = parent_guard.find_child_by_id(state.static_text_size_id);
        state.static_text_time = parent_guard.find_child_by_id(state.static_text_time_id);
        state.static_text_file = parent_guard.find_child_by_id(state.static_text_file_id);
        state.static_text_status = parent_guard.find_child_by_id(state.static_text_status_id);
        state.progress_bar = parent_guard.find_child_by_id(state.progress_bar_id);
    }

    let download_slot = download_manager();
    let mut guard = download_slot
        .lock()
        .expect("download manager slot re-entered on the GUI thread");
    if guard.is_none() {
        *guard = Some(DownloadManager::new());
    }
}

pub fn download_menu_shutdown(_layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    set_download_manager(None);
    let state_handle = download_menu_state();
    let mut state = state_handle.borrow_mut();
    state.parent = None;
    state.static_text_size = None;
    state.static_text_time = None;
    state.static_text_file = None;
    state.static_text_status = None;
    state.progress_bar = None;
    state.last_update = None;
    state.time_left = 0;
}

pub fn download_menu_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let state_handle = download_menu_state();
    let mut state = state_handle.borrow_mut();
    let events = {
        let download_slot = download_manager();
        let mut guard = download_slot
            .lock()
            .expect("download manager slot re-entered on the GUI thread");
        guard
            .as_mut()
            .map(|manager| manager.update())
            .unwrap_or_default()
    };
    for event in events {
        update_from_event(&mut state, event, receiving_manager, effects);
    }
    update_time_text(&mut state);
}

pub fn download_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
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

    cancel_download(window, effects);
    WindowMsgHandled::Handled
}

pub fn download_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let state_handle = download_menu_state();
            let cancel_id = state_handle.borrow().button_cancel_id;
            if control_id == cancel_id {
                cancel_download(window, effects);
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
    let download_slot = download_manager();
    let mut guard = download_slot
        .lock()
        .expect("download manager slot re-entered on the GUI thread");
    if let Some(manager) = guard.as_mut() {
        manager.queue_file_for_download(download);
    }
}

pub fn start_next_download() {
    let download_slot = download_manager();
    let mut guard = download_slot
        .lock()
        .expect("download manager slot re-entered on the GUI thread");
    if let Some(manager) = guard.as_mut() {
        let _ = manager.download_next_queued_file();
    }
}
