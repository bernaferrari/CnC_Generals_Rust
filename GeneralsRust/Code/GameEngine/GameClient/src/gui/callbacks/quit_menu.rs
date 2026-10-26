//! QuitMenu.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::message_box::{
    MessageBoxFunc, message_box_yes_no, quit_message_box_yes_no,
};
use crate::gui::callbacks::popup_save_load::show_live_popup_save_load_layout;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};
use crate::gui::{get_lan_setup, hide_diplomacy, hide_in_game_chat, show_shell_map_if_available};
use crate::helpers::TheControlBar;
use crate::message_stream::{GameMessage, GameMessageType};
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

#[derive(Default)]
pub struct QuitMenuState {
    quit_menu_layout: Option<Rc<RefCell<WindowLayout>>>,
    full_quit_menu_layout: Option<Rc<RefCell<WindowLayout>>>,
    no_save_quit_menu_layout: Option<Rc<RefCell<WindowLayout>>>,
    save_load_layout: Option<Rc<RefCell<WindowLayout>>>,
    is_visible: bool,
    residual_visible: bool,
    residual_action: u8,
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

impl QuitMenuState {
    pub(crate) fn is_visible(&self) -> bool {
        self.is_visible && self.quit_menu_layout.is_some()
    }
}

fn init_gadgets_full_quit(
    state: &mut QuitMenuState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    state.button_exit = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonExit") as i32;
    state.button_restart = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonRestart") as i32;
    state.button_return = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonReturn") as i32;
    state.button_options = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonOptions") as i32;
    state.button_save_load = NameKeyGenerator::name_to_key("QuitMenu.wnd:ButtonSaveLoad") as i32;

    (|manager: &mut crate::gui::window_manager::WindowManager| {
        state.button_restart_win = manager.get_window_by_id(state.button_restart);
        state.button_save_load_win = manager.get_window_by_id(state.button_save_load);
        state.button_options_win = manager.get_window_by_id(state.button_options);
        state.button_exit_win = manager.get_window_by_id(state.button_exit);
    })(receiving_manager);
}

fn init_gadgets_no_save_quit(
    state: &mut QuitMenuState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    state.button_exit = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonExit") as i32;
    state.button_restart = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonRestart") as i32;
    state.button_return = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonReturn") as i32;
    state.button_options = NameKeyGenerator::name_to_key("QuitNoSave.wnd:ButtonOptions") as i32;
    state.button_save_load = -1;

    (|manager: &mut crate::gui::window_manager::WindowManager| {
        state.button_restart_win = manager.get_window_by_id(state.button_restart);
        state.button_options_win = manager.get_window_by_id(state.button_options);
        state.button_save_load_win = None;
        state.button_exit_win = manager.get_window_by_id(state.button_exit);
    })(receiving_manager);
}

fn send_back_button_selection(
    button_name: &str,
    effects: &mut WindowEventEffects<'_>,
    selected: Option<&GameWindow>,
) -> bool {
    let button_id = NameKeyGenerator::name_to_key(button_name) as i32;
    effects.with_manager(|manager, effects| {
        let Some(button) = manager.get_window_by_id_with_selected(button_id, selected) else {
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
            effects,
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

pub fn destroy_quit_menu(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    let (full_layout, no_save_layout) = effects.with_quit_menu(|state| {
        state.quit_confirmation_window = None;
        let layouts = (
            state.full_quit_menu_layout.take(),
            state.no_save_quit_menu_layout.take(),
        );
        state.quit_menu_layout = None;
        state.is_visible = false;
        layouts
    });

    // Load confirmation can call this from a WindowManager input callback.
    // Queue destruction so real QuitMenu layouts are torn down after that
    // dispatch instead of silently failing the manager's re-entry guard.
    for layout in [full_layout, no_save_layout].into_iter().flatten() {
        effects.queue_window_operation(move |manager, effects| {
            manager.destroy_layout(&layout, effects)
        });
    }
}

/// Retire only this client's layouts, then discard its old control receipts.
/// Deferred window destruction is admitted by the same Effects completion
/// boundary before the receiving GameClient continues resetting.
pub(crate) fn reset_quit_menu(effects: &mut WindowEventEffects<'_>) {
    destroy_quit_menu(effects);
    effects.with_quit_menu(|state| *state = QuitMenuState::default());
}

fn exit_quit_menu(effects: &mut WindowEventEffects<'_>) {
    destroy_quit_menu(effects);

    if TheGameLogic::is_in_multiplayer_game()
        && !TheGameLogic::is_in_skirmish_game()
        && !session_is_sandbox()
    {
        let local_player = crate::message_stream::player_state::get_local_player_id() as u32;
        effects
            .emitted_messages
            .push(GameMessage::new(GameMessageType::SelfDestruct(
                local_player,
            )));
    }

    effects
        .emitted_messages
        .push(GameMessage::new(GameMessageType::ClearGameData));

    if !TheGameLogic::is_in_multiplayer_game() {
        TheGameLogic::set_game_paused(false, true);
    }
    effects.ui_mut().set_client_quiet(true);
}

fn no_exit_quit_menu(effects: &mut WindowEventEffects<'_>) {
    effects.with_quit_menu(|state| state.quit_confirmation_window = None);
}

#[allow(dead_code)] // C++ parity: GUI callback, will be wired to menu system
fn quit_to_desktop_quit_menu(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    destroy_quit_menu(effects);

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
    effects.ui_mut().set_client_quiet(true);
}

fn surrender_quit_menu(effects: &mut WindowEventEffects<'_>) {
    destroy_quit_menu(effects);

    if TheVictoryConditions::is_local_allied_victory() {
        return;
    }

    let local_player = crate::message_stream::player_state::get_local_player_id() as u32;
    effects
        .emitted_messages
        .push(GameMessage::new(GameMessageType::SelfDestruct(
            local_player,
        )));

    effects.ui_mut().set_client_quiet(true);
}

fn restart_mission_menu(effects: &mut WindowEventEffects<'_>) {
    destroy_quit_menu(effects);

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
        let mut msg = GameMessage::new(GameMessageType::NewGame);
        msg.append_integer_argument(game_mode);
        msg.append_integer_argument(diff);
        msg.append_integer_argument(rank_points);
        msg.append_integer_argument(fps);
        effects.emitted_messages.push(msg);
        init_random_with_seed(0);
    }

    effects.ui_mut().set_client_quiet(true);
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

pub fn hide_quit_menu(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
    effects: &mut WindowEventEffects<'_>,
) {
    let hidden = effects.with_quit_menu(|state| {
        if !state.is_visible {
            return None;
        }
        let group = state.quit_menu_layout.as_ref().map(|layout| {
            if state
                .no_save_quit_menu_layout
                .as_ref()
                .is_some_and(|no_save| Rc::ptr_eq(no_save, layout))
            {
                "QuitNoSaveBack"
            } else {
                "QuitFullBack"
            }
        });
        Some(group)
    });
    let Some(group) = hidden else {
        return;
    };
    if let Some(group) = group {
        receiving_manager.transition_reverse(group, selected_window.as_deref_mut());
    }
    effects.ui_mut().set_quit_menu_visible(false);
    let confirmation = effects.with_quit_menu(|state| {
        state.is_visible = false;
        state.quit_confirmation_window.take()
    });
    if let Some(window) = confirmation {
        let _ = receiving_manager.destroy_window(window);
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
pub fn toggle_quit_menu(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) {
    let _ = toggle_quit_menu_with_result(effects, selected_window.as_deref_mut());
}

/// Run retail `ToggleQuitMenu()` and report which of its three externally
/// visible routes occurred.  See [`QuitMenuToggleResult`].
pub fn toggle_quit_menu_with_result(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) -> QuitMenuToggleResult {
    toggle_quit_menu_receiving(
        TheScriptEngine::is_game_ending(),
        effects,
        selected_window.as_deref_mut(),
    )
}

/// Receiving operation observes the driving interpreter before GUI callbacks.
pub fn toggle_quit_menu_receiving(
    game_ending: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) -> QuitMenuToggleResult {
    toggle_quit_menu_with_selected(game_ending, effects, selected_window.as_deref_mut())
}

fn toggle_quit_menu_with_selected(
    game_ending: bool,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) -> QuitMenuToggleResult {
    if TheGameLogic::is_intro_movie_playing() || TheGameLogic::is_loading_map() || game_ending {
        return QuitMenuToggleResult::Suppressed;
    }

    if effects.receiving_disconnect_menu_visible() {
        return QuitMenuToggleResult::Suppressed;
    }

    let owner = effects
        .window_manager_owner
        .expect("QuitMenu requires receiving Shell owner")
        .clone();
    let options = owner
        .with_shell_ref(|shell| shell.options_layout_receipt())
        .flatten();
    if let Some(layout) = options.filter(|layout| !layout.borrow().is_hidden()) {
        if !send_back_button_selection("OptionsMenu.wnd:ButtonBack", effects, selected.as_deref()) {
            let mut immediate = false;
            layout
                .borrow()
                .run_shutdown_with_effects(Some(&mut immediate as &mut dyn std::any::Any), effects);
            owner.with_shell_mut(|shell| shell.finish_options_shutdown_receipt(&layout, immediate));
            effects.with_manager(|manager, _effects| {
                layout.borrow().hide_with_manager_and_selected(
                    manager,
                    true,
                    selected.as_deref_mut(),
                );
            });
        }
        return QuitMenuToggleResult::ClosedInterveningLayout;
    }

    let popup = effects.with_quit_menu(|state| state.save_load_layout.clone());
    if let Some(layout) = popup.filter(|layout| !layout.borrow().is_hidden()) {
        if !send_back_button_selection("PopupSaveLoad.wnd:ButtonBack", effects, selected.as_deref())
        {
            effects.with_manager(|manager, _| layout.borrow_mut().hide(manager, true));
        }
        effects.with_quit_menu(|state| state.save_load_layout = None);
        return QuitMenuToggleResult::ClosedInterveningLayout;
    }

    let (visible, layout, no_save) = effects.with_quit_menu(|state| {
        (
            state.is_visible,
            state.quit_menu_layout.clone(),
            state.no_save_quit_menu_layout.clone(),
        )
    });
    if visible && layout.is_some() {
        let confirmation = effects.with_quit_menu(|state| {
            state.is_visible = false;
            state.quit_confirmation_window.take()
        });
        if let Some(window) = confirmation {
            effects.with_manager(|manager, effects| {
                let _ = manager.destroy_window(window);
            });
        }
        if !TheGameLogic::is_in_multiplayer_game() {
            TheGameLogic::set_game_paused(false, true);
        }
        if let Some(layout) = layout.as_ref() {
            let group = if no_save
                .as_ref()
                .is_some_and(|no_save| Rc::ptr_eq(no_save, layout))
            {
                "QuitNoSaveBack"
            } else {
                "QuitFullBack"
            };
            effects.with_manager(|manager, effects| {
                manager.transition_reverse(group, selected.as_deref_mut())
            });
            // C++ plays the WindowTransitions Quit{Full,NoSave}Back reverse
            // animation, which ends with the layout windows HIDDEN. This host
            // has no such transition group data, so reverse alone left the
            // menu painted over the resumed match (ButtonReturn resumed the
            // world while the panel stayed on screen). Force the net retail
            // end state: every layout window hidden immediately.
            effects.with_manager(|manager, _effects| {
                layout.borrow().hide_with_manager_and_selected(
                    manager,
                    true,
                    selected.as_deref_mut(),
                )
            });
        }
    } else {
        effects.ui_mut().set_cursor_arrow();
        TheControlBar::hide_purchase_science();

        let in_multiplayer = TheGameLogic::is_in_multiplayer_game();
        let in_replay = TheGameLogic::is_in_replay_game();
        if in_multiplayer || in_replay {
            if effects.with_quit_menu(|state| state.no_save_quit_menu_layout.is_none()) {
                let created = effects.with_manager(|manager, effects| {
                    manager
                        .create_layout_with_windows("Menus/QuitNoSave.wnd", effects)
                        .ok()
                        .map(|(layout, _)| layout)
                });
                effects.with_quit_menu(|state| state.no_save_quit_menu_layout = created);
            }
            effects.with_quit_menu(|state| {
                state.quit_menu_layout = state.no_save_quit_menu_layout.clone()
            });
            effects.with_manager(|receiving_manager, effects| {
                effects.with_quit_menu(|state| init_gadgets_no_save_quit(state, receiving_manager))
            });
            effects.with_manager(|manager, effects| {
                manager.transition_remove("QuitNoSave", false, selected.as_deref_mut());
                manager.transition_set_group("QuitNoSave", false, selected.as_deref_mut());
            });
        } else {
            if effects.with_quit_menu(|state| state.full_quit_menu_layout.is_none()) {
                let created = effects.with_manager(|manager, effects| {
                    manager
                        .create_layout_with_windows("Menus/QuitMenu.wnd", effects)
                        .ok()
                        .map(|(layout, _)| layout)
                });
                effects.with_quit_menu(|state| state.full_quit_menu_layout = created);
            }
            effects.with_quit_menu(|state| {
                state.quit_menu_layout = state.full_quit_menu_layout.clone()
            });
            effects.with_manager(|receiving_manager, effects| {
                effects.with_quit_menu(|state| init_gadgets_full_quit(state, receiving_manager))
            });
            effects.with_manager(|manager, effects| {
                manager.transition_remove("QuitFull", false, selected.as_deref_mut());
                manager.transition_set_group("QuitFull", false, selected.as_deref_mut());
            });
        }

        let layout = effects.with_quit_menu(|state| state.quit_menu_layout.clone());
        let Some(layout) = layout else {
            effects.with_quit_menu(|state| state.is_visible = false);
            effects.ui_mut().set_quit_menu_visible(false);
            return QuitMenuToggleResult::LayoutUnavailable;
        };

        layout.borrow().run_init(None);
        let (save_button, options_button, restart_button, exit_button) =
            effects.with_quit_menu(|state| {
                (
                    state.button_save_load_win.clone(),
                    state.button_options_win.clone(),
                    state.button_restart_win.clone(),
                    state.button_exit_win.clone(),
                )
            });

        if !effects.ui().get_input_enabled() {
            if let Some(save) = save_button.as_ref() {
                let _ = save.borrow_mut().enable(false);
            }
            if let Some(options) = options_button.as_ref() {
                let _ = options.borrow_mut().enable(false);
            }
        } else {
            if let Some(save) = save_button.as_ref() {
                let _ = save.borrow_mut().enable(true);
            }
            if let Some(options) = options_button.as_ref() {
                let _ = options.borrow_mut().enable(true);
            }
        }

        if TheGameLogic::is_in_multiplayer_game() || TheGameLogic::is_in_skirmish_game() {
            if let Some(restart) = restart_button.as_ref() {
                let _ = restart.borrow_mut().enable(true);
            }
            if !TheGameLogic::is_in_skirmish_game() {
                if let Some(restart) = restart_button.as_ref() {
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
                if let Some(restart) = restart_button.as_ref() {
                    let _ = restart.borrow_mut().enable(false);
                }
            }
        } else {
            if let Some(restart) = restart_button.as_ref() {
                let _ = restart.borrow_mut().enable(true);
            }
            if !TheGameLogic::is_in_replay_game() {
                if let Some(restart) = restart_button.as_ref() {
                    let _ = restart
                        .borrow_mut()
                        .set_text(&GameText::fetch("GUI:RestartMission"));
                }
                if let Some(exit) = exit_button.as_ref() {
                    let _ = exit
                        .borrow_mut()
                        .set_text(&GameText::fetch("GUI:ExitMission"));
                }
            }
            TheGameLogic::set_game_paused(true, true);
        }

        if let Some(window) = effects.with_quit_menu(|state| state.quit_confirmation_window.take())
        {
            effects.with_manager(|manager, effects| {
                let _ = manager.destroy_window(window);
            });
        }

        let _ = effects.with_manager(|manager, _effects| hide_diplomacy(false, manager, _effects));
        let _ = effects.with_manager(|receiving_manager, effects| {
            hide_in_game_chat(false, receiving_manager, effects)
        });
        TheControlBar::hide_purchase_science();
        effects.with_quit_menu(|state| state.is_visible = true);
    }

    let visible = effects.with_quit_menu(|state| state.is_visible);
    effects.ui_mut().set_quit_menu_visible(visible);
    QuitMenuToggleResult::ToggledQuitMenu
}

/// True only while a live `QuitMenu.wnd` / `QuitNoSave.wnd` callback layout is
/// visible. This deliberately excludes the residual smoke-test latch: Main
/// uses it to keep its authoritative offline simulation pause in step with a
/// real WND callback.
pub fn is_quit_menu_visible(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_quit_menu(|state| state.is_visible && state.quit_menu_layout.is_some())
}

pub fn quit_menu_system(
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    _effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    quit_menu_system_with_camera_input(msg, data1, None, _effects, Some(_window))
}
pub(crate) fn select_quit_options_with_camera_input(
    control_id: WindowMsgData,
    observation: crate::gui::callbacks::OptionsCameraInputObservation,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    quit_menu_system_with_camera_input(
        WindowMessage::GadgetSelected,
        control_id,
        Some(observation),
        effects,
        None,
    )
}
fn quit_menu_system_with_camera_input(
    msg: WindowMessage,
    data1: WindowMsgData,
    camera_input: Option<crate::gui::callbacks::OptionsCameraInputObservation>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    selected: Option<&mut GameWindow>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let (
                button_save_load,
                button_exit,
                button_return,
                button_options,
                button_restart,
                existing_layout,
            ) = effects.with_quit_menu(|state| {
                (
                    state.button_save_load,
                    state.button_exit,
                    state.button_return,
                    state.button_options,
                    state.button_restart,
                    state.save_load_layout.clone(),
                )
            });

            if control_id == button_save_load {
                if let Some(layout) = existing_layout {
                    // This callback normally runs under WindowManager input
                    // dispatch, so the popup init must not re-enter the
                    // manager for control lookup.
                    layout.borrow().run_init(None);
                    show_live_popup_save_load_layout(layout, effects);
                } else {
                    // The current input dispatch already owns WindowManager.
                    // Queue the retail layout creation instead of invoking
                    // with_window_manager and getting its fail-closed result.
                    effects.queue_window_operation(move |manager, effects| {
                        let Some(layout) = manager
                            .create_layout_with_windows("Menus/PopupSaveLoad.wnd", effects)
                            .ok()
                            .map(|(layout, _)| layout)
                        else {
                            return;
                        };

                        effects
                            .with_quit_menu(|state| state.save_load_layout = Some(layout.clone()));

                        layout.borrow().run_init(None);
                        show_live_popup_save_load_layout(layout, effects);
                    });
                }
                return WindowMsgHandled::Handled;
            } else if control_id == button_exit {
                let yes: MessageBoxFunc = Box::new(exit_quit_menu);
                let no: MessageBoxFunc = Box::new(no_exit_quit_menu);
                let confirmation = effects.with_manager(|manager, _effects| {
                    quit_message_box_yes_no(
                        &GameText::fetch("GUI:QuitPopupTitle"),
                        &GameText::fetch("GUI:QuitPopupMessage"),
                        Some(yes),
                        Some(no),
                        manager,
                        _effects,
                    )
                });
                effects.with_quit_menu(|state| state.quit_confirmation_window = confirmation);
            } else if control_id == button_return {
                // The state phase has ended before the return button enters
                // the retail toggle path and its synchronous window callbacks.
                let _ = toggle_quit_menu_with_selected(
                    TheScriptEngine::is_game_ending(),
                    effects,
                    selected,
                );
                return WindowMsgHandled::Handled;
            } else if control_id == button_options {
                crate::gui::shell::base::queue_shell_operation_with_effects(
                    effects,
                    move |shell, effects| {
                        if let Some(layout) = shell.get_options_layout(true) {
                            let _ = layout.run_init_with_effects(
                                camera_input
                                    .as_ref()
                                    .map(|value| value as &dyn std::any::Any),
                                effects,
                            );
                            layout.hide_with_effects(false, effects);
                            layout.bring_forward_with_effects(effects);
                        }
                    },
                );
            } else if control_id == button_restart {
                if TheGameLogic::is_in_multiplayer_game() {
                    let yes: MessageBoxFunc = Box::new(surrender_quit_menu);
                    let no: MessageBoxFunc = Box::new(no_exit_quit_menu);
                    let confirmation = effects.with_manager(|manager, _effects| {
                        message_box_yes_no(
                            &GameText::fetch("GUI:SurrenderConfirmationTitle"),
                            &GameText::fetch("GUI:SurrenderConfirmation"),
                            Some(yes),
                            Some(no),
                            manager,
                            _effects,
                        )
                    });
                    effects.with_quit_menu(|state| state.quit_confirmation_window = confirmation);
                } else {
                    let yes: MessageBoxFunc = Box::new(restart_mission_menu);
                    let no: MessageBoxFunc = Box::new(no_exit_quit_menu);
                    let confirmation = effects.with_manager(|manager, _effects| {
                        message_box_yes_no(
                            &GameText::fetch("GUI:RestartConfirmationTitle"),
                            &GameText::fetch("GUI:RestartConfirmation"),
                            Some(yes),
                            Some(no),
                            manager,
                            _effects,
                        )
                    });
                    effects.with_quit_menu(|state| state.quit_confirmation_window = confirmation);
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

/// Residual observations remain scoped to the same receiving client.
pub fn residual_quit_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualQuitMenuAction {
    effects.with_quit_menu(|state| match state.residual_action {
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
    })
}
pub fn residual_quit_menu_is_visible(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_quit_menu(|state| state.residual_visible)
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
pub fn simulate_quit_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_quit_menu(|state| {
        ensure_quit_control_ids(state);
        // NameKeyGenerator may return 0 without dictionary; bind residual still succeeds.
        let _ = (
            state.button_exit,
            state.button_return,
            state.button_options,
            state.button_restart,
            state.button_save_load,
        );
        true
    })
}

/// Residual: show QuitMenu without loading QuitMenu.wnd layouts.
pub fn simulate_quit_menu_toggle_show(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_quit_menu(|state| {
        ensure_quit_control_ids(state);
        state.is_visible = true;
        state.residual_visible = true;
        state.residual_action = ResidualQuitMenuAction::ToggleShow as u8;
        state.residual_visible
    })
}

/// Residual: hide QuitMenu without destroying layouts.
pub fn simulate_quit_menu_toggle_hide(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_quit_menu(|state| {
        state.is_visible = false;
        state.residual_visible = false;
        state.residual_action = ResidualQuitMenuAction::ToggleHide as u8;
        !state.residual_visible
    })
}

/// Residual: fire ButtonExit without confirmation dialog / ClearGameData.
pub fn simulate_quit_menu_exit_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_quit_menu(|state| {
        ensure_quit_control_ids(state);
        state.residual_action = ResidualQuitMenuAction::Exit as u8;
        true
    })
}

/// Residual: fire ButtonReturn (resume) without full toggle side effects.
pub fn simulate_quit_menu_return_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_quit_menu(|state| {
        ensure_quit_control_ids(state);
        state.is_visible = false;
        state.residual_visible = false;
        state.residual_action = ResidualQuitMenuAction::Return as u8;
        true
    })
}

/// Residual: fire ButtonOptions without loading Options layout.
pub fn simulate_quit_menu_options_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_quit_menu(|state| {
        ensure_quit_control_ids(state);
        state.residual_action = ResidualQuitMenuAction::Options as u8;
        true
    })
}

/// Residual: fire ButtonRestart without restart/surrender confirmation.
pub fn simulate_quit_menu_restart_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_quit_menu(|state| {
        ensure_quit_control_ids(state);
        state.residual_action = ResidualQuitMenuAction::Restart as u8;
        true
    })
}

/// Residual: fire ButtonSaveLoad without PopupSaveLoad layout create.
pub fn simulate_quit_menu_save_load_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_quit_menu(|state| {
        ensure_quit_control_ids(state);
        state.residual_action = ResidualQuitMenuAction::SaveLoad as u8;
        true
    })
}

/// Residual: confirm exit path without ClearGameData / SelfDestruct.
pub fn simulate_quit_menu_confirm_exit(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_quit_menu(|state| {
        state.is_visible = false;
        state.quit_confirmation_window = None;
        state.residual_visible = false;
        state.residual_action = ResidualQuitMenuAction::ConfirmExit as u8;
        true
    })
}

/// Residual: destroy residual latch (no live layout teardown required).
pub fn simulate_quit_menu_destroy(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_quit_menu(|state| {
        state.is_visible = false;
        state.quit_confirmation_window = None;
        state.residual_visible = false;
        state.residual_action = ResidualQuitMenuAction::Destroy as u8;
        true
    })
}

/// Residual: show + Exit + ConfirmExit composite (clean exit honesty).
pub fn simulate_quit_menu_prepare_exit(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_quit_menu_bind_controls(effects) {
        return false;
    }
    if !simulate_quit_menu_toggle_show(effects) {
        return false;
    }
    if !simulate_quit_menu_exit_button_gadget_selected(effects) {
        return false;
    }
    simulate_quit_menu_confirm_exit(effects)
}

/// Human click-through: OS LeftDown/Up on a retail `QuitMenu.wnd:*` gadget
/// (C++ WindowXlat hit → GBM_SELECTED). Not `simulate_*` first.
fn drive_os_wnd_quit_named(
    name: &str,
    latch: impl FnOnce(&mut WindowEventEffects<'_>) -> bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !crate::gui::dispatch_os_click_named_window(name, effects) {
        return false;
    }
    latch(effects)
}

pub fn drive_os_wnd_quit_menu_exit_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonExit",
        simulate_quit_menu_exit_button_gadget_selected,
        effects,
    )
}

pub fn drive_os_wnd_quit_menu_return_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonReturn",
        simulate_quit_menu_return_button_gadget_selected,
        effects,
    )
}

pub fn drive_os_wnd_quit_menu_options_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonOptions",
        simulate_quit_menu_options_button_gadget_selected,
        effects,
    )
}

pub fn drive_os_wnd_quit_menu_restart_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonRestart",
        simulate_quit_menu_restart_button_gadget_selected,
        effects,
    )
}

pub fn drive_os_wnd_quit_menu_save_load_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_quit_named(
        "QuitMenu.wnd:ButtonSaveLoad",
        simulate_quit_menu_save_load_button_gadget_selected,
        effects,
    )
}

/// Create `Menus/QuitMenu.wnd` if missing, then require live SaveLoad gadget.
///
/// C++ Pause/Esc shows this layout. Fail-closed if parse/create fails.
pub fn ensure_live_quit_menu_layout(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    const SAVE_LOAD: &str = "QuitMenu.wnd:ButtonSaveLoad";
    let present = (|manager: &mut crate::gui::window_manager::WindowManager| {
        manager.find_window_by_name(SAVE_LOAD).is_some()
    })(receiving_manager)
        || (|manager: &mut crate::gui::window_manager::WindowManager| {
            manager
                .create_layout_with_windows("Menus/QuitMenu.wnd", effects)
                .is_ok()
        })(receiving_manager);
    if !present
        || !(|manager: &mut crate::gui::window_manager::WindowManager| {
            manager.find_window_by_name(SAVE_LOAD).is_some()
        })(receiving_manager)
    {
        return false;
    }

    // The WND file has no LAYOUTINIT callback. Bind the QuitMenu IDs here so
    // a real ButtonSaveLoad click reaches `quit_menu_system` rather than only
    // a residual test latch.
    effects.with_quit_menu(|state| init_gadgets_full_quit(state, receiving_manager));
    true
}

/// Human click-through: ButtonExit then confirm residual (C++ exit + yes).
pub fn drive_os_wnd_quit_menu_prepare_exit_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked = drive_os_wnd_quit_menu_exit_like_cpp(effects);
    if !clicked {
        return false;
    }
    simulate_quit_menu_confirm_exit(effects)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    use crate::gui::callbacks::popup_save_load::{
        drive_os_wnd_popup_save_load_save_like_cpp, live_popup_save_load_window_present,
    };
    fn with_fixture(operation: impl FnOnce(&mut WindowEventEffects<'_>)) {
        let owner = crate::gui::window_manager::WindowManagerOwner::new();
        let mut menus = crate::gui::callbacks::menu_callbacks::MenuManager::new();
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.window_manager_owner = Some(&owner);
        effects.menus = Some(menus.receiving_menus());
        operation(&mut effects);
    }

    fn install_named_button(name: &str, x: i32, y: i32, effects: &mut WindowEventEffects<'_>) {
        effects.with_manager(|manager, effects| {
            let button = manager
                .create_window(None, x, y, 80, 24, effects)
                .expect(name);
            button.borrow_mut().set_name(name);
            button.borrow_mut().hide(false, manager);
        });
    }

    #[test]
    fn os_wnd_quit_menu_exit_hits_button_then_latches() {
        with_fixture(|effects| {
            install_named_button("QuitMenu.wnd:ButtonExit", 10, 10, effects);
            assert!(
                drive_os_wnd_quit_menu_exit_like_cpp(effects),
                "OS WND click on ButtonExit must latch Exit residual"
            );
            assert_eq!(
                residual_quit_menu_last_action(effects),
                ResidualQuitMenuAction::Exit
            );
            assert!(!drive_os_wnd_quit_menu_return_like_cpp(effects));
        });
    }

    #[test]
    fn os_wnd_quit_menu_return_and_options_hit_named_gadgets() {
        with_fixture(|effects| {
            install_named_button("QuitMenu.wnd:ButtonReturn", 10, 40, effects);
            install_named_button("QuitMenu.wnd:ButtonOptions", 10, 70, effects);
            assert!(drive_os_wnd_quit_menu_return_like_cpp(effects));
            assert_eq!(
                residual_quit_menu_last_action(effects),
                ResidualQuitMenuAction::Return
            );
            assert!(drive_os_wnd_quit_menu_options_like_cpp(effects));
            assert_eq!(
                residual_quit_menu_last_action(effects),
                ResidualQuitMenuAction::Options
            );
        });
    }

    #[test]
    fn os_wnd_quit_menu_prepare_exit_hits_exit_then_confirms() {
        with_fixture(|effects| {
            install_named_button("QuitMenu.wnd:ButtonExit", 10, 100, effects);
            assert!(drive_os_wnd_quit_menu_prepare_exit_like_cpp(effects));
            assert_eq!(
                residual_quit_menu_last_action(effects),
                ResidualQuitMenuAction::ConfirmExit
            );
            assert!(!residual_quit_menu_is_visible(effects));
        });
    }

    #[test]
    fn retail_quit_save_load_click_creates_initialized_popup_layout() {
        with_fixture(|effects| {
            effects.with_manager(|manager, effects| manager.reset(effects));
            assert!(
                effects.with_manager(|manager, effects| ensure_live_quit_menu_layout(
                    manager, effects
                )),
                "the retail QuitMenu.wnd must parse with ButtonSaveLoad"
            );

            assert!(
                drive_os_wnd_quit_menu_save_load_like_cpp(effects),
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
                    effects.with_manager(|manager, _| live_popup_save_load_window_present(
                        name, manager
                    )),
                    "real PopupSaveLoad.wnd must create {name}"
                );
            }

            let popup_state =
                crate::gui::callbacks::popup_save_load::prepare_live_popup_save_load_for_click(
                    effects,
                );
            assert!(
                popup_state,
                "popup must have bound its real listbox/control state"
            );

            assert!(
                drive_os_wnd_popup_save_load_save_like_cpp(effects),
                "Save must reach the retail PopupSaveLoad callback"
            );
            let save_description_visible = effects.with_manager(|manager, effects| {
                manager
                    .find_window_by_name("PopupSaveLoad.wnd:SaveDescParent")
                    .is_some_and(|window| !window.borrow().is_hidden())
            });
            assert!(
                save_description_visible,
                "New Save Game must open SaveDescParent instead of stopping at ButtonSave"
            );
            assert!(
                crate::gui::dispatch_os_click_named_window(
                    "PopupSaveLoad.wnd:ButtonSaveDescCancel",
                    effects
                ),
                "the real save description cancel button must be hittable"
            );
            effects.with_manager(|manager, effects| manager.reset(effects));
        });
    }

    #[test]
    fn retail_quit_menu_toggle_tracks_live_wnd_visibility() {
        with_fixture(|effects| {
            // This is intentionally the production toggle, not the residual
            // `simulate_quit_menu_*` latch.  It parses the retail QuitMenu.wnd,
            // binds its actual ButtonReturn gadget, and exercises the visible →
            // hidden pause transition used by Main's offline host bridge.
            effects.with_manager(|manager, effects| manager.reset(effects));
            effects.with_quit_menu(|state| *state = QuitMenuState::default());
            TheGameLogic::set_intro_movie_playing(false);
            TheGameLogic::set_game_paused(false, true);

            assert_eq!(
                toggle_quit_menu_with_result(effects, None),
                QuitMenuToggleResult::ToggledQuitMenu
            );
            assert!(is_quit_menu_visible(effects));
            assert!(TheGameLogic::is_game_paused());
            assert!(effects.with_manager(|manager, effects| {
                manager
                    .find_window_by_name("QuitMenu.wnd:ButtonReturn")
                    .is_some_and(|window| !window.borrow().is_hidden())
            }));

            assert!(
                crate::gui::dispatch_os_click_named_window("QuitMenu.wnd:ButtonReturn", effects),
                "the retail Return button must dispatch through QuitMenuSystem"
            );
            assert!(!is_quit_menu_visible(effects));
            assert!(!TheGameLogic::is_game_paused());

            effects.with_quit_menu(|state| *state = QuitMenuState::default());
            effects.with_manager(|manager, effects| manager.reset(effects));
        });
    }
}

#[cfg(test)]
mod receiving_script_ending_tests {
    use super::*;
    #[test]
    fn receiving_quit_ending_suppresses_before_window_or_pause_changes() {
        let mut menus = super::super::menu_callbacks::MenuManager::new();
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menus = Some(menus.receiving_menus());
        let visible = is_quit_menu_visible(&mut effects);
        assert_eq!(
            toggle_quit_menu_receiving(true, &mut effects, None),
            QuitMenuToggleResult::Suppressed
        );
        assert_eq!(is_quit_menu_visible(&mut effects), visible);
    }

    #[test]
    fn quit_state_and_residuals_are_instance_owned() {
        let mut a = super::super::menu_callbacks::MenuManager::new();
        let mut b = super::super::menu_callbacks::MenuManager::new();
        let mut messages_a = Vec::new();
        let mut messages_b = Vec::new();
        let mut a_effects = WindowEventEffects::new(&mut messages_a);
        let mut b_effects = WindowEventEffects::new(&mut messages_b);
        a_effects.menus = Some(a.receiving_menus());
        b_effects.menus = Some(b.receiving_menus());
        assert!(simulate_quit_menu_toggle_show(&mut a_effects));
        assert!(!residual_quit_menu_is_visible(&mut b_effects));
        assert_eq!(
            residual_quit_menu_last_action(&mut b_effects),
            ResidualQuitMenuAction::None
        );
        assert!(simulate_quit_menu_destroy(&mut b_effects));
        assert!(residual_quit_menu_is_visible(&mut a_effects));
        assert_eq!(
            residual_quit_menu_last_action(&mut a_effects),
            ResidualQuitMenuAction::ToggleShow
        );
        // Both are only residual latches, not real visible WNDs.
        assert!(!is_quit_menu_visible(&mut a_effects));
        assert!(!is_quit_menu_visible(&mut b_effects));
    }
}
