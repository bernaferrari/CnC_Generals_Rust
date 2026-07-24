//! QuitMenu.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::message_box::{
    message_box_yes_no, quit_message_box_yes_no, MessageBoxFunc,
};
use crate::gui::{
    get_disconnect_menu, get_lan_setup, get_shell, hide_diplomacy, hide_in_game_chat,
    show_shell_map_if_available, try_with_shell_mut,
};
use crate::gui::{
    with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
};
use crate::helpers::{TheControlBar, TheInGameUI};
use crate::message_stream::{get_message_stream, GameMessageType};
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
    if let Some(layout) = state.full_quit_menu_layout.take() {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
    if let Some(layout) = state.no_save_quit_menu_layout.take() {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
    state.quit_menu_layout = None;
    state.is_visible = false;
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

pub fn toggle_quit_menu() {
    if TheGameLogic::is_intro_movie_playing()
        || TheGameLogic::is_loading_map()
        || TheScriptEngine::is_game_ending()
    {
        return;
    }

    if let Ok(menu) = get_disconnect_menu().read() {
        if menu.is_visible() {
            return;
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
        return;
    }

    {
        let state_handle = quit_menu_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(layout) = state.save_load_layout.as_ref() {
            if !layout.borrow().is_hidden() {
                if send_back_button_selection("PopupSaveLoad.wnd:ButtonBack") {
                    state.save_load_layout = None;
                    return;
                }
                layout.borrow_mut().hide(true);
                state.save_load_layout = None;
                return;
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
            return;
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
                if state.save_load_layout.is_none() {
                    let created = with_window_manager(|manager| {
                        manager
                            .create_layout_with_windows("Menus/PopupSaveLoad.wnd")
                            .ok()
                            .map(|(layout, _)| layout)
                    });
                    state.save_load_layout = created;
                }
                if let Some(layout) = state.save_load_layout.as_ref() {
                    layout.borrow().run_init(None);
                    layout.borrow_mut().hide(false);
                    layout.borrow_mut().bring_forward();
                }
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
                toggle_quit_menu();
            } else if control_id == state.button_options {
                let _ = try_with_shell_mut(|shell| {
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
