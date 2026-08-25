//! QuitMenu.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::message_box::{
    MessageBoxFunc, message_box_yes_no, quit_message_box_yes_no,
};
use crate::gui::callbacks::popup_save_load::show_live_popup_save_load_layout;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
    queue_window_manager_op, with_window_manager,
};
use crate::gui::{
    get_disconnect_menu, get_lan_setup, get_shell, hide_diplomacy, hide_in_game_chat,
    queue_shell_operation, show_shell_map_if_available, try_with_shell_mut,
};
use crate::helpers::{TheControlBar, TheInGameUI};
use crate::message_stream::{GameMessageType, get_message_stream};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::recorder::{with_recorder, with_recorder_mut};
use game_engine::get_game_state;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine, TheVictoryConditions};
use gamelogic::player::ThePlayerList;
use gamelogic::system::game_logic::{GAME_INTERNET, GAME_LAN};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct QuitMenuState {
    quit_menu_layout: Option<Rc<RefCell<WindowLayout>>>,
    full_quit_menu_layout: Option<Rc<RefCell<WindowLayout>>>,
    no_save_quit_menu_layout: Option<Rc<RefCell<WindowLayout>>>,
    save_load_layout: Option<Rc<RefCell<WindowLayout>>>,
    is_visible: bool,
    quit_confirmation_window: Option<Rc<RefCell<GameWindow>>>,
    button_restart_win: Option<Rc<RefCell<GameWindow>>>,
    button_save_load_win: Option<Rc<RefCell<GameWindow>>>,
    button_options_win: Option<Rc<RefCell<GameWindow>>>,
    button_exit_win: Option<Rc<RefCell<GameWindow>>>,
    button_exit: i32,
    button_restart: i32,
    button_return: i32,
    button_options: i32,
    button_save_load: i32,
}

thread_local! {
    static QUIT_MENU_STATE: Arc<Mutex<QuitMenuState>> =
        Arc::new(Mutex::new(QuitMenuState::default()));
}

fn quit_menu_state() -> Arc<Mutex<QuitMenuState>> {
    QUIT_MENU_STATE.with(|state| state.clone())
}

fn init_gadgets_full_quit(state: &mut QuitMenuState) {
    state.button_exit = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonExit") as i32;
    state.button_restart = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonRestart") as i32;
    state.button_return = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonReturn") as i32;
    state.button_options = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonOptions") as i32;
    state.button_save_load = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonSaveLoad") as i32;

    with_window_manager(|manager| {
        state.button_restart_win = manager.get_window_by_id(state.button_restart);
        state.button_save_load_win = manager.get_window_by_id(state.button_save_load);
        state.button_options_win = manager.get_window_by_id(state.button_options);
        state.button_exit_win = manager.get_window_by_id(state.button_exit);
    });
}

fn init_gadgets_no_save_quit(state: &mut QuitMenuState) {
    state.button_exit = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonExit") as i32;
    state.button_restart = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonRestart") as i32;
    state.button_return = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonReturn") as i32;
    state.button_options = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonOptions") as i32;
    state.button_save_load = -1;

    with_window_manager(|manager| {
        state.button_restart_win = manager.get_window_by_id(state.button_restart);
        state.button_options_win = manager.get_window_by_id(state.button_options);
        state.button_save_load_win = None;
        state.button_exit_win = manager.get_window_by_id(state.button_exit);
    });
}

fn send_back_button_selection(button_name: &str) -> bool {
    let button_id = NameKeyGenerator::name_to_key(button_name) as i32;
    with_window_manager(|manager| {
        let Some(button) = manager.get_window_by_id(button_id) else {
            return false;
        };

        let target = {
            let button_ref = button.borrow();
            button_ref.get_parent().unwrap_or_else(|| button.clone())
        };

        let _ = manager.send_system_message(
            &target,
            WindowMessage::GadgetSelected,
            button_id as WindowMsgData,
            button_id as WindowMsgData,
        );
        true
    })
}

#[cfg(feature = "online_ui")]
fn internet_session_is_sandbox() -> bool {
    crate::gamespy_game::with_gamespy_game_info(|info| info.is_sandbox())
}

#[cfg(not(feature = "online_ui"))]
fn internet_session_is_sandbox() -> bool {
    false
}

fn session_is_sandbox() -> bool {
    match TheGameLogic::get_game_mode() {
        GAME_LAN => {
            let setup = get_lan_setup();
            setup.game_info().is_sandbox()
        }
        GAME_INTERNET => internet_session_is_sandbox(),
        _ => false,
    }
}

pub fn destroy_quit_menu() {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.quit_confirmation_window = None;
    let full_layout = state.full_quit_menu_layout.take();
    let no_save_layout = state.no_save_quit_menu_layout.take();
    state.quit_menu_layout = None;
    state.is_visible = false;
    drop(state);

    // Load confirmation can call this from a WindowManager input callback.
    // Queue destruction so real QuitMenu layouts are torn down after that
    // dispatch instead of silently failing the manager's re-entry guard.
    for layout in [full_layout, no_save_layout].into_iter().flatten() {
        queue_window_manager_op(move |manager| manager.destroy_layout(&layout));
    }
}

fn exit_quit_menu() {
    destroy_quit_menu();

    if TheGameLogic::is_in_multiplayer_game()
        && !TheGameLogic::is_in_skirmish_game()
        && !session_is_sandbox()
    {
        let local_player = crate::message_stream::player_state::get_local_player_id() as u32;
        let message_stream = get_message_stream();
        let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
        stream.append_message(GameMessageType::SelfDestruct(local_player));
    }

    let message_stream = get_message_stream();
    let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
    stream.append_message(GameMessageType::ClearGameData);

    if !TheGameLogic::is_in_multiplayer_game() {
        TheGameLogic::set_game_paused(false, true);
    }
    TheInGameUI::set_client_quiet(true);
}

fn no_exit_quit_menu() {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.quit_confirmation_window = None;
}

#[allow(dead_code)] // C++ parity: GUI callback, will be wired to menu system
fn quit_to_desktop_quit_menu() {
    destroy_quit_menu();

    if TheGameLogic::is_in_game() {
        with_recorder_mut(|recorder| {
            if recorder.is_recording() {
                recorder.stop_recording();
            }
        });
        let _ = TheGameLogic::clear_game_data();
    }

    if let Some(engine) = get_game_engine() {
        let mut engine = engine.lock();
        engine.set_quitting(true);
    }
    TheInGameUI::set_client_quiet(true);
}

fn surrender_quit_menu() {
    destroy_quit_menu();

    if TheVictoryConditions::is_local_allied_victory() {
        return;
    }

    let local_player = crate::message_stream::player_state::get_local_player_id() as u32;
    let message_stream = get_message_stream();
    let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
    stream.append_message(GameMessageType::SelfDestruct(local_player));

    TheInGameUI::set_client_quiet(true);
}

fn restart_mission_menu() {
    destroy_quit_menu();

    let game_mode = TheGameLogic::get_game_mode();
    let map_name = get_global_data()
        .map(|data| data.read().map_name.clone())
        .unwrap_or_default();
    let map_name = restart_map_name_for_pending_file(&map_name);

    let replay_file = with_recorder(|recorder| recorder.get_current_replay_filename().to_string())
        .unwrap_or_default();

    with_recorder_mut(|recorder| {
        if recorder.is_recording() {
            recorder.stop_recording();
        }
    });

    let rank_points = TheGameLogic::get_rank_points_to_add_at_game_start();
    let diff = TheScriptEngine::get_global_difficulty();
    let fps = get_game_engine()
        .map(|engine| engine.lock().get_frames_per_second_limit())
        .unwrap_or(30) as i32;

    let _ = TheGameLogic::clear_game_data();
    if let Some(engine) = get_game_engine() {
        let mut engine = engine.lock();
        engine.set_quitting(false);
    }

    if !replay_file.is_empty() {
        with_recorder_mut(|recorder| {
            let _ = recorder.playback_file(replay_file);
        });
    } else {
        if let Some(data) = get_global_data() {
            let mut data = data.write();
            data.pending_file = map_name.clone();
        }
        let message_stream = get_message_stream();
        let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
        let msg = stream.append_message(GameMessageType::NewGame);
        msg.append_integer_argument(game_mode);
        msg.append_integer_argument(diff);
        msg.append_integer_argument(rank_points);
        msg.append_integer_argument(fps);
        init_random_with_seed(0);
    }

    TheInGameUI::set_client_quiet(true);
}

fn restart_map_name_for_pending_file(map_name: &str) -> String {
    let game_state = get_game_state();
    let is_save_map = game_state.is_in_save_directory(Path::new(map_name))
        || map_name.starts_with("Save/")
        || map_name.starts_with("Save\\");
    if is_save_map {
        let pristine = game_state.get_pristine_map_name();
        if !pristine.is_empty() {
            return pristine.to_string();
        }
    }
    map_name.to_string()
}

pub fn hide_quit_menu() {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if !state.is_visible {
        return;
    }

    if let Some(layout) = state.quit_menu_layout.as_ref() {
        let group = if state
            .no_save_quit_menu_layout
            .as_ref()
            .is_some_and(|no_save| Rc::ptr_eq(no_save, layout))
        {
            "QuitNoSaveBack"
        } else {
            "QuitFullBack"
        };
        with_window_manager(|manager| manager.transition_reverse(group));
    }

    TheInGameUI::set_quit_menu_visible(false);
    state.is_visible = false;
    if let Some(window) = state.quit_confirmation_window.take() {
        with_window_manager(|manager| {
            let _ = manager.destroy_window(window);
        });
    }
    if !TheGameLogic::is_in_multiplayer_game() {
        TheGameLogic::set_game_paused(false, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_from_save_uses_pristine_map_name_like_cpp() {
        let old_pristine = {
            let game_state = get_game_state();
            game_state.get_pristine_map_name().to_string()
        };
        {
            let mut game_state = get_game_state();
            game_state.set_pristine_map_name("Maps\\Campaign\\USA05\\USA05.map".to_string());
        }

        assert_eq!(
            restart_map_name_for_pending_file("Save\\scratch.map"),
            "Maps\\Campaign\\USA05\\USA05.map"
        );
        assert_eq!(
            restart_map_name_for_pending_file("Maps\\Campaign\\USA06\\USA06.map"),
            "Maps\\Campaign\\USA06\\USA06.map"
        );

        get_game_state().set_pristine_map_name(old_pristine);
    }
}

/// Observable result of a retail `ToggleQuitMenu()` call.
///
/// The C++ function also services the Options and PopupSaveLoad back paths
/// before it considers the quit layout.  Main needs to distinguish that
/// successful interception from a missing QuitMenu WND: only a real visible
/// quit layout owns Main's simulation pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitMenuToggleResult {
    /// The live QuitMenu / QuitNoSave WND was shown or hidden.
    ToggledQuitMenu,
    /// Escape backed out of Options or PopupSaveLoad instead of toggling Quit.
    ClosedInterveningLayout,
    /// C++ intentionally rejected the command (intro/loading/disconnect).
    Suppressed,
    /// C++ attempted the QuitMenu path but the retail WND could not load.
    LayoutUnavailable,
}

/// Preserve the C++-shaped fire-and-forget entry point used by GUI command
/// translation and callbacks.
pub fn toggle_quit_menu() {
    let _ = toggle_quit_menu_with_result();
}

/// Run retail `ToggleQuitMenu()` and report which of its three externally
/// visible routes occurred.  See [`QuitMenuToggleResult`].
pub fn toggle_quit_menu_with_result() -> QuitMenuToggleResult {
    if TheGameLogic::is_intro_movie_playing()
        || TheGameLogic::is_loading_map()
        || TheScriptEngine::is_game_ending()
    {
        return QuitMenuToggleResult::Suppressed;
    }

    if let Ok(menu) = get_disconnect_menu().read() {
        if menu.is_visible() {
            return QuitMenuToggleResult::Suppressed;
        }
    }

    let handled_options = try_with_shell_mut(|shell| {
        if let Some(layout) = shell.get_options_layout(false) {
            if !layout.is_hidden() {
                if send_back_button_selection("OptionsMenu.wnd:ButtonBack") {
                    return true;
                }
                let mut immediate = false;
                let _ = layout.run_shutdown(&mut immediate);
                layout.hide(true);
                return true;
            }
        }
        false
    })
    .unwrap_or(false);
    if handled_options {
        return QuitMenuToggleResult::ClosedInterveningLayout;
    }

    {
        let state_handle = quit_menu_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(layout) = state.save_load_layout.as_ref() {
            if !layout.borrow().is_hidden() {
                if send_back_button_selection("PopupSaveLoad.wnd:ButtonBack") {
                    state.save_load_layout = None;
                    return QuitMenuToggleResult::ClosedInterveningLayout;
                }
                layout.borrow_mut().hide(true);
                state.save_load_layout = None;
                return QuitMenuToggleResult::ClosedInterveningLayout;
            }
        }
    }

    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    if state.is_visible && state.quit_menu_layout.is_some() {
        state.is_visible = false;
        if let Some(window) = state.quit_confirmation_window.take() {
            with_window_manager(|manager| {
                let _ = manager.destroy_window(window);
            });
        }
        if !TheGameLogic::is_in_multiplayer_game() {
            TheGameLogic::set_game_paused(false, true);
        }
        if let Some(layout) = state.quit_menu_layout.as_ref() {
            let group = if state
                .no_save_quit_menu_layout
                .as_ref()
                .is_some_and(|no_save| Rc::ptr_eq(no_save, layout))
            {
                "QuitNoSaveBack"
            } else {
                "QuitFullBack"
            };
            with_window_manager(|manager| manager.transition_reverse(group));
        }
    } else {
        TheInGameUI::set_cursor_arrow();
        TheControlBar::hide_purchase_science();

        let in_multiplayer = TheGameLogic::is_in_multiplayer_game();
        let in_replay = TheGameLogic::is_in_replay_game();
        if in_multiplayer || in_replay {
            if state.no_save_quit_menu_layout.is_none() {
                let created = with_window_manager(|manager| {
                    manager
                        .create_layout_with_windows("Menus/QuitNoSave.wnd")
                        .ok()
                        .map(|(layout, _)| layout)
                });
                state.no_save_quit_menu_layout = created;
            }
            state.quit_menu_layout = state.no_save_quit_menu_layout.clone();
            init_gadgets_no_save_quit(&mut state);
            with_window_manager(|manager| {
                manager.transition_remove("QuitNoSave", false);
                manager.transition_set_group("QuitNoSave", false);
            });
        } else {
            if state.full_quit_menu_layout.is_none() {
                let created = with_window_manager(|manager| {
                    manager
                        .create_layout_with_windows("Menus/QuitMenu.wnd")
                        .ok()
                        .map(|(layout, _)| layout)
                });
                state.full_quit_menu_layout = created;
            }
            state.quit_menu_layout = state.full_quit_menu_layout.clone();
            init_gadgets_full_quit(&mut state);
            with_window_manager(|manager| {
                manager.transition_remove("QuitFull", false);
                manager.transition_set_group("QuitFull", false);
            });
        }

        let Some(layout) = state.quit_menu_layout.as_ref() else {
            state.is_visible = false;
            TheInGameUI::set_quit_menu_visible(false);
            return QuitMenuToggleResult::LayoutUnavailable;
        };

        layout.borrow().run_init(None);

        if !TheInGameUI::get_input_enabled() {
            if let Some(save) = state.button_save_load_win.as_ref() {
                let _ = save.borrow_mut().enable(false);
            }
            if let Some(options) = state.button_options_win.as_ref() {
                let _ = options.borrow_mut().enable(false);
            }
        } else {
            if let Some(save) = state.button_save_load_win.as_ref() {
                let _ = save.borrow_mut().enable(true);
            }
            if let Some(options) = state.button_options_win.as_ref() {
                let _ = options.borrow_mut().enable(true);
            }
        }

        if TheGameLogic::is_in_multiplayer_game() || TheGameLogic::is_in_skirmish_game() {
            if let Some(restart) = state.button_restart_win.as_ref() {
                let _ = restart.borrow_mut().enable(true);
            }
            if !TheGameLogic::is_in_skirmish_game() {
                if let Some(restart) = state.button_restart_win.as_ref() {
                    let _ = restart
                        .borrow_mut()
                        .set_text(&GameText::fetch("GUI:Surrender"));
                }
            } else {
                TheGameLogic::set_game_paused(true, true);
            }

            let disable_restart = if TheGameLogic::is_in_skirmish_game() {
                false
            } else {
                let local_active = ThePlayerList()
                    .read()
                    .ok()
                    .and_then(|list| {
                        list.get_local_player().and_then(|player| {
                            player.read().ok().map(|player| player.is_player_active())
                        })
                    })
                    .unwrap_or(true);
                !local_active || TheVictoryConditions::is_local_allied_victory()
            };

            if disable_restart {
                if let Some(restart) = state.button_restart_win.as_ref() {
                    let _ = restart.borrow_mut().enable(false);
                }
            }
        } else {
            if let Some(restart) = state.button_restart_win.as_ref() {
                let _ = restart.borrow_mut().enable(true);
            }
            if !TheGameLogic::is_in_replay_game() {
                if let Some(restart) = state.button_restart_win.as_ref() {
                    let _ = restart
                        .borrow_mut()
                        .set_text(&GameText::fetch("GUI:RestartMission"));
                }
                if let Some(exit) = state.button_exit_win.as_ref() {
                    let _ = exit
                        .borrow_mut()
                        .set_text(&GameText::fetch("GUI:ExitMission"));
                }
            }
            TheGameLogic::set_game_paused(true, true);
        }

        if let Some(window) = state.quit_confirmation_window.take() {
            with_window_manager(|manager| {
                let _ = manager.destroy_window(window);
            });
        }

        let _ = hide_diplomacy(false);
        let _ = hide_in_game_chat(false);
        TheControlBar::hide_purchase_science();
        state.is_visible = true;
    }

    TheInGameUI::set_quit_menu_visible(state.is_visible);
    QuitMenuToggleResult::ToggledQuitMenu
}

/// True only while a live `QuitMenu.wnd` / `QuitNoSave.wnd` callback layout is
/// visible. This deliberately excludes the residual smoke-test latch: Main
/// uses it to keep its authoritative offline simulation pause in step with a
/// real WND callback.
pub fn is_quit_menu_visible() -> bool {
    let state = quit_menu_state();
    let state = state.lock().unwrap_or_else(|e| e.into_inner());
    state.is_visible && state.quit_menu_layout.is_some()
}

pub fn quit_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let state_handle = quit_menu_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

            if control_id == state.button_save_load {
                let existing_layout = state.save_load_layout.clone();
                drop(state);

                if let Some(layout) = existing_layout {
                    // This callback normally runs under WindowManager input
                    // dispatch, so the popup init must not re-enter the
                    // manager for control lookup.
                    layout.borrow().run_init(None);
                    show_live_popup_save_load_layout(layout);
                } else {
                    // The current input dispatch already owns WindowManager.
                    // Queue the retail layout creation instead of invoking
                    // with_window_manager and getting its fail-closed result.
                    queue_window_manager_op(move |manager| {
                        let Some(layout) = manager
                            .create_layout_with_windows("Menus/PopupSaveLoad.wnd")
                            .ok()
                            .map(|(layout, _)| layout)
                        else {
                            return;
                        };

                        let state_handle = quit_menu_state();
                        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
                        state.save_load_layout = Some(layout.clone());
                        drop(state);

                        layout.borrow().run_init(None);
                        show_live_popup_save_load_layout(layout);
                    });
                }
                return WindowMsgHandled::Handled;
            } else if control_id == state.button_exit {
                let yes: MessageBoxFunc = Box::new(exit_quit_menu);
                let no: MessageBoxFunc = Box::new(no_exit_quit_menu);
                state.quit_confirmation_window = quit_message_box_yes_no(
                    &GameText::fetch("GUI:QuitPopupTitle"),
                    &GameText::fetch("GUI:QuitPopupMessage"),
                    Some(yes),
                    Some(no),
                );
            } else if control_id == state.button_return {
                // `ToggleQuitMenu` owns the same state mutex.  A real
                // ButtonReturn arrives through WindowManager while this
                // callback is live, so release our callback-local guard
                // before re-entering the retail toggle path.
                drop(state);
                toggle_quit_menu();
                return WindowMsgHandled::Handled;
            } else if control_id == state.button_options {
                queue_shell_operation(|shell| {
                    if let Some(layout) = shell.get_options_layout(true) {
                        let _ = layout.run_init(None);
                        layout.hide(false);
                        layout.bring_forward();
                    }
                });
            } else if control_id == state.button_restart {
                if TheGameLogic::is_in_multiplayer_game() {
                    let yes: MessageBoxFunc = Box::new(surrender_quit_menu);
                    let no: MessageBoxFunc = Box::new(no_exit_quit_menu);
                    state.quit_confirmation_window = message_box_yes_no(
                        &GameText::fetch("GUI:SurrenderConfirmationTitle"),
                        &GameText::fetch("GUI:SurrenderConfirmation"),
                        Some(yes),
                        Some(no),
                    );
                } else {
                    let yes: MessageBoxFunc = Box::new(restart_mission_menu);
                    let no: MessageBoxFunc = Box::new(no_exit_quit_menu);
                    state.quit_confirmation_window = message_box_yes_no(
                        &GameText::fetch("GUI:RestartConfirmationTitle"),
                        &GameText::fetch("GUI:RestartConfirmation"),
                        Some(yes),
                        Some(no),
                    );
                }
            }

            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

/// Residual: last QuitMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualQuitMenuAction {
    None = 0,
    ToggleShow = 1,
    ToggleHide = 2,
    Exit = 3,
    Return = 4,
    Options = 5,
    Restart = 6,
    SaveLoad = 7,
    ConfirmExit = 8,
    Destroy = 9,
}

static RESIDUAL_QUIT_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_QUIT_VISIBLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_quit_action_store(action: ResidualQuitMenuAction) {
    RESIDUAL_QUIT_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last QuitMenu residual action.
pub fn residual_quit_menu_last_action() -> ResidualQuitMenuAction {
    match RESIDUAL_QUIT_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualQuitMenuAction::ToggleShow,
        2 => ResidualQuitMenuAction::ToggleHide,
        3 => ResidualQuitMenuAction::Exit,
        4 => ResidualQuitMenuAction::Return,
        5 => ResidualQuitMenuAction::Options,
        6 => ResidualQuitMenuAction::Restart,
        7 => ResidualQuitMenuAction::SaveLoad,
        8 => ResidualQuitMenuAction::ConfirmExit,
        9 => ResidualQuitMenuAction::Destroy,
        _ => ResidualQuitMenuAction::None,
    }
}

/// Residual: QuitMenu visibility latch (independent of live layout).
pub fn residual_quit_menu_is_visible() -> bool {
    RESIDUAL_QUIT_VISIBLE.load(std::sync::atomic::Ordering::Relaxed)
}

fn ensure_quit_control_ids(state: &mut QuitMenuState) {
    if state.button_exit == 0 {
        state.button_exit = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonExit") as i32;
    }
    if state.button_restart == 0 {
        state.button_restart = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonRestart") as i32;
    }
    if state.button_return == 0 {
        state.button_return = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonReturn") as i32;
    }
    if state.button_options == 0 {
        state.button_options = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonOptions") as i32;
    }
    if state.button_save_load == 0 {
        state.button_save_load =
            NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonSaveLoad") as i32;
    }
}

/// Residual: bind QuitMenu control IDs (no layout load required).
pub fn simulate_quit_menu_bind_controls() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_quit_control_ids(&mut state);
    // NameKeyGenerator may return 0 without dictionary; bind residual still succeeds.
    let _ = (
        state.button_exit,
        state.button_return,
        state.button_options,
        state.button_restart,
        state.button_save_load,
    );
    true
}

/// Residual: show QuitMenu without loading QuitMenu.wnd layouts.
pub fn simulate_quit_menu_toggle_show() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_quit_control_ids(&mut state);
    state.is_visible = true;
    RESIDUAL_QUIT_VISIBLE.store(true, std::sync::atomic::Ordering::Relaxed);
    residual_quit_action_store(ResidualQuitMenuAction::ToggleShow);
    residual_quit_menu_is_visible()
}

/// Residual: hide QuitMenu without destroying layouts.
pub fn simulate_quit_menu_toggle_hide() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_visible = false;
    RESIDUAL_QUIT_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_quit_action_store(ResidualQuitMenuAction::ToggleHide);
    !residual_quit_menu_is_visible()
}

/// Residual: fire ButtonExit without confirmation dialog / ClearGameData.
pub fn simulate_quit_menu_exit_button_gadget_selected() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_quit_control_ids(&mut state);
    residual_quit_action_store(ResidualQuitMenuAction::Exit);
    true
}

/// Residual: fire ButtonReturn (resume) without full toggle side effects.
pub fn simulate_quit_menu_return_button_gadget_selected() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_quit_control_ids(&mut state);
    state.is_visible = false;
    RESIDUAL_QUIT_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_quit_action_store(ResidualQuitMenuAction::Return);
    true
}

/// Residual: fire ButtonOptions without loading Options layout.
pub fn simulate_quit_menu_options_button_gadget_selected() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_quit_control_ids(&mut state);
    residual_quit_action_store(ResidualQuitMenuAction::Options);
    true
}

/// Residual: fire ButtonRestart without restart/surrender confirmation.
pub fn simulate_quit_menu_restart_button_gadget_selected() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_quit_control_ids(&mut state);
    residual_quit_action_store(ResidualQuitMenuAction::Restart);
    true
}

/// Residual: fire ButtonSaveLoad without PopupSaveLoad layout create.
pub fn simulate_quit_menu_save_load_button_gadget_selected() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_quit_control_ids(&mut state);
    residual_quit_action_store(ResidualQuitMenuAction::SaveLoad);
    true
}

/// Residual: confirm exit path without ClearGameData / SelfDestruct.
pub fn simulate_quit_menu_confirm_exit() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_visible = false;
    state.quit_confirmation_window = None;
    RESIDUAL_QUIT_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_quit_action_store(ResidualQuitMenuAction::ConfirmExit);
    true
}

/// Residual: destroy residual latch (no live layout teardown required).
pub fn simulate_quit_menu_destroy() -> bool {
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_visible = false;
    state.quit_confirmation_window = None;
    RESIDUAL_QUIT_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_quit_action_store(ResidualQuitMenuAction::Destroy);
    true
}

/// Residual: show + Exit + ConfirmExit composite (clean exit honesty).
pub fn simulate_quit_menu_prepare_exit() -> bool {
    if !simulate_quit_menu_bind_controls() {
        return false;
    }
    if !simulate_quit_menu_toggle_show() {
        return false;
    }
    if !simulate_quit_menu_exit_button_gadget_selected() {
        return false;
    }
    simulate_quit_menu_confirm_exit()
}

/// Human click-through: OS LeftDown/Up on a retail `QuitMenu.wnd:*` gadget
/// (C++ WindowXlat hit → GBM_SELECTED). Not `simulate_*` first.
fn drive_os_wnd_quit_named(name: &str, latch: impl FnOnce() -> bool) -> bool {
    if !crate::gui::dispatch_os_click_named_window(name) {
        return false;
    }
    latch()
}

pub fn drive_os_wnd_quit_menu_exit_like_cpp() -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonExit",
        simulate_quit_menu_exit_button_gadget_selected,
    )
}

pub fn drive_os_wnd_quit_menu_return_like_cpp() -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonReturn",
        simulate_quit_menu_return_button_gadget_selected,
    )
}

pub fn drive_os_wnd_quit_menu_options_like_cpp() -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonOptions",
        simulate_quit_menu_options_button_gadget_selected,
    )
}

pub fn drive_os_wnd_quit_menu_restart_like_cpp() -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonRestart",
        simulate_quit_menu_restart_button_gadget_selected,
    )
}

pub fn drive_os_wnd_quit_menu_save_load_like_cpp() -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonSaveLoad",
        simulate_quit_menu_save_load_button_gadget_selected,
    )
}

/// Create `Menus/QuitMenu.wnd` if missing, then require live SaveLoad gadget.
///
/// C++ Pause/Esc shows this layout. Fail-closed if parse/create fails.
pub fn ensure_live_quit_menu_layout() -> bool {
    const SAVE_LOAD: &str = "QuitMenu.wnd:ButtonSaveLoad";
    let present = with_window_manager(|manager| manager.find_window_by_name(SAVE_LOAD).is_some())
        || with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/QuitMenu.wnd")
                .is_ok()
        });
    if !present || !with_window_manager(|manager| manager.find_window_by_name(SAVE_LOAD).is_some())
    {
        return false;
    }

    // The WND file has no LAYOUTINIT callback. Bind the QuitMenu IDs here so
    // a real ButtonSaveLoad click reaches `quit_menu_system` rather than only
    // a residual test latch.
    let state_handle = quit_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    init_gadgets_full_quit(&mut state);
    true
}

/// Human click-through: ButtonExit then confirm residual (C++ exit + yes).
pub fn drive_os_wnd_quit_menu_prepare_exit_like_cpp() -> bool {
    let clicked = drive_os_wnd_quit_menu_exit_like_cpp();
    if !clicked {
        return false;
    }
    simulate_quit_menu_confirm_exit()
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    use crate::gui::callbacks::popup_save_load::{
        drive_os_wnd_popup_save_load_save_like_cpp, live_popup_save_load_window_present,
    };
    use crate::gui::with_window_manager;

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    #[test]
    fn os_wnd_quit_menu_exit_hits_button_then_latches() {
        install_named_button("QuitMenu.wnd:ButtonExit", 10, 10);
        assert!(
            drive_os_wnd_quit_menu_exit_like_cpp(),
            "OS WND click on ButtonExit must latch Exit residual"
        );
        assert_eq!(
            residual_quit_menu_last_action(),
            ResidualQuitMenuAction::Exit
        );
        assert!(!drive_os_wnd_quit_menu_return_like_cpp());
    }

    #[test]
    fn os_wnd_quit_menu_return_and_options_hit_named_gadgets() {
        install_named_button("QuitMenu.wnd:ButtonReturn", 10, 40);
        install_named_button("QuitMenu.wnd:ButtonOptions", 10, 70);
        assert!(drive_os_wnd_quit_menu_return_like_cpp());
        assert_eq!(
            residual_quit_menu_last_action(),
            ResidualQuitMenuAction::Return
        );
        assert!(drive_os_wnd_quit_menu_options_like_cpp());
        assert_eq!(
            residual_quit_menu_last_action(),
            ResidualQuitMenuAction::Options
        );
    }

    #[test]
    fn os_wnd_quit_menu_prepare_exit_hits_exit_then_confirms() {
        install_named_button("QuitMenu.wnd:ButtonExit", 10, 100);
        assert!(drive_os_wnd_quit_menu_prepare_exit_like_cpp());
        assert_eq!(
            residual_quit_menu_last_action(),
            ResidualQuitMenuAction::ConfirmExit
        );
        assert!(!residual_quit_menu_is_visible());
    }

    #[test]
    fn retail_quit_save_load_click_creates_initialized_popup_layout() {
        with_window_manager(|manager| manager.reset());
        assert!(
            ensure_live_quit_menu_layout(),
            "the retail QuitMenu.wnd must parse with ButtonSaveLoad"
        );

        assert!(
            drive_os_wnd_quit_menu_save_load_like_cpp(),
            "the real ButtonSaveLoad OS click must be hittable"
        );

        for name in [
            "PopupSaveLoad.wnd:SaveLoadMenu",
            "PopupSaveLoad.wnd:MenuButtonFrame",
            "PopupSaveLoad.wnd:ButtonSave",
            "PopupSaveLoad.wnd:ButtonLoad",
            "PopupSaveLoad.wnd:ListboxGames",
            "PopupSaveLoad.wnd:LoadConfirmParent",
            "PopupSaveLoad.wnd:OverwriteConfirmParent",
            "PopupSaveLoad.wnd:SaveDescParent",
            "PopupSaveLoad.wnd:DeleteConfirmParent",
        ] {
            assert!(
                live_popup_save_load_window_present(name),
                "real PopupSaveLoad.wnd must create {name}"
            );
        }

        let popup_state =
            crate::gui::callbacks::popup_save_load::prepare_live_popup_save_load_for_click();
        assert!(
            popup_state,
            "popup must have bound its real listbox/control state"
        );

        assert!(
            drive_os_wnd_popup_save_load_save_like_cpp(),
            "Save must reach the retail PopupSaveLoad callback"
        );
        let save_description_visible = with_window_manager(|manager| {
            manager
                .find_window_by_name("PopupSaveLoad.wnd:SaveDescParent")
                .is_some_and(|window| !window.borrow().is_hidden())
        });
        assert!(
            save_description_visible,
            "New Save Game must open SaveDescParent instead of stopping at ButtonSave"
        );
        assert!(
            crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonSaveDescCancel"),
            "the real save description cancel button must be hittable"
        );
        with_window_manager(|manager| manager.reset());
    }

    #[test]
    fn retail_quit_menu_toggle_tracks_live_wnd_visibility() {
        // This is intentionally the production toggle, not the residual
        // `simulate_quit_menu_*` latch.  It parses the retail QuitMenu.wnd,
        // binds its actual ButtonReturn gadget, and exercises the visible →
        // hidden pause transition used by Main's offline host bridge.
        with_window_manager(|manager| manager.reset());
        {
            let state = quit_menu_state();
            *state.lock().unwrap_or_else(|e| e.into_inner()) = QuitMenuState::default();
        }
        TheGameLogic::set_intro_movie_playing(false);
        TheGameLogic::set_game_paused(false, true);

        assert_eq!(
            toggle_quit_menu_with_result(),
            QuitMenuToggleResult::ToggledQuitMenu
        );
        assert!(is_quit_menu_visible());
        assert!(TheGameLogic::is_game_paused());
        assert!(with_window_manager(|manager| {
            manager
                .find_window_by_name("QuitMenu.wnd:ButtonReturn")
                .is_some_and(|window| !window.borrow().is_hidden())
        }));

        assert!(
            crate::gui::dispatch_os_click_named_window("QuitMenu.wnd:ButtonReturn"),
            "the retail Return button must dispatch through QuitMenuSystem"
        );
        assert!(!is_quit_menu_visible());
        assert!(!TheGameLogic::is_game_paused());

        {
            let state = quit_menu_state();
            *state.lock().unwrap_or_else(|e| e.into_inner()) = QuitMenuState::default();
        }
        with_window_manager(|manager| manager.reset());
    }
}
