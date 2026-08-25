//! Layout and per-window script callback binding (FunctionLexicon parity).
#![allow(unused_imports)]

use crate::gui::callbacks::menu_callbacks::MenuCallbacks;
use crate::gui::callbacks::{
    beacon_window_input, challenge_menu_init, challenge_menu_input, challenge_menu_shutdown,
    challenge_menu_system, challenge_menu_update, difficulty_select_init, difficulty_select_input,
    difficulty_select_system, download_menu_init, download_menu_input, download_menu_shutdown,
    download_menu_system, download_menu_update, game_info_window_init, game_info_window_system,
    generals_exp_points_input, generals_exp_points_system, get_control_bar_system,
    get_diplomacy_system, get_ingame_ui_system, get_menu_manager, get_message_box_system,
    ime_candidate_main_draw, ime_candidate_text_area_draw, ime_candidate_window_input,
    ime_candidate_window_system, in_game_popup_message_init, in_game_popup_message_input,
    in_game_popup_message_system, keyboard_options_menu_init, keyboard_options_menu_input,
    keyboard_options_menu_shutdown, keyboard_options_menu_system, keyboard_options_menu_update,
    lan_game_options_menu_init, lan_game_options_menu_input, lan_game_options_menu_shutdown,
    lan_game_options_menu_system, lan_game_options_menu_update, lan_map_select_menu_init,
    lan_map_select_menu_input, lan_map_select_menu_shutdown, lan_map_select_menu_system,
    lan_map_select_menu_update, network_direct_connect_init, network_direct_connect_input,
    network_direct_connect_shutdown, network_direct_connect_system, network_direct_connect_update,
    popup_buddy_notification_system, popup_communicator_init, popup_communicator_input,
    popup_communicator_shutdown, popup_communicator_system, popup_communicator_update,
    popup_host_game_init, popup_host_game_input, popup_host_game_system, popup_host_game_update,
    popup_join_game_init, popup_join_game_input, popup_join_game_system, popup_ladder_select_init,
    popup_ladder_select_input, popup_ladder_select_shutdown, popup_ladder_select_system,
    popup_ladder_select_update, popup_player_info_init, popup_player_info_input,
    popup_player_info_shutdown, popup_player_info_system, popup_player_info_update,
    popup_replay_init, popup_replay_input, popup_replay_shutdown, popup_replay_system,
    popup_replay_update, quit_menu_system, rc_game_details_menu_init, rc_game_details_menu_system,
    replay_menu_init, replay_menu_input, replay_menu_shutdown, replay_menu_system,
    replay_menu_update, save_load_menu_full_screen_init, save_load_menu_init, save_load_menu_input,
    save_load_menu_shutdown, save_load_menu_system, save_load_menu_update, score_screen_init,
    score_screen_input, score_screen_shutdown, score_screen_system, score_screen_update,
    skirmish_game_options_menu_init, skirmish_game_options_menu_input,
    skirmish_game_options_menu_shutdown, skirmish_game_options_menu_system,
    skirmish_game_options_menu_update, skirmish_map_select_menu_init,
    skirmish_map_select_menu_input, skirmish_map_select_menu_shutdown,
    skirmish_map_select_menu_system, skirmish_map_select_menu_update, wol_buddy_overlay_init,
    wol_buddy_overlay_input, wol_buddy_overlay_rc_menu_init, wol_buddy_overlay_rc_menu_system,
    wol_buddy_overlay_shutdown, wol_buddy_overlay_system, wol_buddy_overlay_update,
    wol_custom_score_screen_init, wol_custom_score_screen_input, wol_custom_score_screen_shutdown,
    wol_custom_score_screen_system, wol_custom_score_screen_update, wol_game_setup_menu_init,
    wol_game_setup_menu_input, wol_game_setup_menu_shutdown, wol_game_setup_menu_system,
    wol_game_setup_menu_update, wol_ladder_screen_init, wol_ladder_screen_input,
    wol_ladder_screen_shutdown, wol_ladder_screen_system, wol_ladder_screen_update,
    wol_lobby_menu_init, wol_lobby_menu_input, wol_lobby_menu_shutdown, wol_lobby_menu_system,
    wol_lobby_menu_update, wol_locale_select_init, wol_locale_select_input,
    wol_locale_select_shutdown, wol_locale_select_system, wol_locale_select_update,
    wol_login_menu_init, wol_login_menu_input, wol_login_menu_shutdown, wol_login_menu_system,
    wol_login_menu_update, wol_map_select_menu_init, wol_map_select_menu_input,
    wol_map_select_menu_shutdown, wol_map_select_menu_system, wol_map_select_menu_update,
    wol_message_window_init, wol_message_window_input, wol_message_window_shutdown,
    wol_message_window_system, wol_message_window_update, wol_qm_score_screen_init,
    wol_qm_score_screen_input, wol_qm_score_screen_shutdown, wol_qm_score_screen_system,
    wol_qm_score_screen_update, wol_quick_match_menu_init, wol_quick_match_menu_input,
    wol_quick_match_menu_shutdown, wol_quick_match_menu_system, wol_quick_match_menu_update,
    wol_status_menu_init, wol_status_menu_input, wol_status_menu_shutdown, wol_status_menu_system,
    wol_status_menu_update, wol_welcome_menu_init, wol_welcome_menu_input,
    wol_welcome_menu_shutdown, wol_welcome_menu_system, wol_welcome_menu_update,
};
use crate::gui::game_window::*;
use crate::gui::header_template::get_header_template_manager;
use crate::gui::shell::main_menu::get_main_menu;
use crate::gui::w3d_gadget_draw::{
    w3d_cameo_movie_draw, w3d_clock_draw, w3d_command_bar_background_draw,
    w3d_command_bar_foreground_draw, w3d_command_bar_gen_exp_draw, w3d_command_bar_grid_draw,
    w3d_command_bar_help_popup_draw, w3d_command_bar_top_draw, w3d_credits_menu_draw,
    w3d_draw_map_preview, w3d_gadget_check_box_draw, w3d_gadget_check_box_image_draw,
    w3d_gadget_combo_box_draw, w3d_gadget_combo_box_image_draw, w3d_gadget_horizontal_slider_draw,
    w3d_gadget_horizontal_slider_image_draw, w3d_gadget_horizontal_slider_image_draw_a,
    w3d_gadget_horizontal_slider_image_draw_b, w3d_gadget_list_box_draw,
    w3d_gadget_list_box_image_draw, w3d_gadget_progress_bar_draw,
    w3d_gadget_progress_bar_image_draw, w3d_gadget_progress_bar_image_draw_a,
    w3d_gadget_push_button_draw, w3d_gadget_push_button_image_draw, w3d_gadget_radio_button_draw,
    w3d_gadget_radio_button_image_draw, w3d_gadget_static_text_draw,
    w3d_gadget_static_text_image_draw, w3d_gadget_tab_control_draw,
    w3d_gadget_tab_control_image_draw, w3d_gadget_text_entry_draw,
    w3d_gadget_text_entry_image_draw, w3d_gadget_vertical_slider_draw,
    w3d_gadget_vertical_slider_image_draw, w3d_left_hud_draw,
    w3d_main_menu_button_drop_shadow_draw, w3d_main_menu_draw, w3d_main_menu_four_draw,
    w3d_main_menu_map_border, w3d_main_menu_random_text_draw, w3d_metal_bar_menu_draw, w3d_no_draw,
    w3d_power_draw, w3d_power_draw_a, w3d_right_hud_draw, w3d_shell_menu_scheme_draw,
    w3d_thin_border_draw,
};
use crate::gui::window_script::{
    TabControlData as ScriptTabControlData, WindowDefinition, WindowLayoutDefinition,
    parse_window_script,
};
use crate::gui::{get_disconnect_menu, get_establish_connections_menu};
use log::warn;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use super::*;

impl WindowManager {
    pub(crate) fn bind_layout_callbacks(
        &self,
        layout: &mut WindowLayout,
        layout_def: &WindowLayoutDefinition,
    ) {
        if !layout_def.init_callback.is_empty() {
            layout.init_callback = match layout_def.init_callback.as_str() {
                "W3DMainMenuInit" | "MainMenuInit" => Some(Box::new(|layout, _| {
                    apply_w3d_main_menu_runtime_draw_overrides();
                    let mut menu = get_main_menu();
                    if let Err(err) = menu.init(layout, None) {
                        warn!("Main menu init failed: {}", err);
                    }
                })),
                "SinglePlayerMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_single_player_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Single player menu init failed: {}", err);
                    }
                })),
                "OptionsMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_options_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Options menu init failed: {}", err);
                    }
                })),
                "MapSelectMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_map_select_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Map select menu init failed: {}", err);
                    }
                })),
                "CreditsMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_credits_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Credits menu init failed: {}", err);
                    }
                })),
                "LanLobbyMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_lan_lobby_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("LAN lobby menu init failed: {}", err);
                    }
                })),
                "InGamePopupMessageInit" => Some(Box::new(|layout, _| {
                    in_game_popup_message_init(layout, None);
                })),
                "PopupCommunicatorInit" => Some(Box::new(|layout, _| {
                    popup_communicator_init(layout, None);
                })),
                "PopupJoinGameInit" => Some(Box::new(|layout, _| {
                    popup_join_game_init(layout, None);
                })),
                "SaveLoadMenuInit" => Some(Box::new(|layout, data| {
                    save_load_menu_init(layout, data);
                })),
                "SaveLoadMenuFullScreenInit" => Some(Box::new(|layout, data| {
                    save_load_menu_full_screen_init(layout, data);
                })),
                "PopupReplayInit" => Some(Box::new(|layout, data| {
                    popup_replay_init(layout, data);
                })),
                "ReplayMenuInit" => Some(Box::new(|layout, data| {
                    replay_menu_init(layout, data);
                })),
                "ChallengeMenuInit" => Some(Box::new(|layout, data| {
                    challenge_menu_init(layout, data);
                })),
                "DifficultySelectInit" => Some(Box::new(|layout, data| {
                    difficulty_select_init(layout, data);
                })),
                "KeyboardOptionsMenuInit" => Some(Box::new(|layout, data| {
                    keyboard_options_menu_init(layout, data);
                })),
                "GameSpyPlayerInfoOverlayInit" => Some(Box::new(|layout, data| {
                    popup_player_info_init(layout, data);
                })),
                "ScoreScreenInit" => Some(Box::new(|layout, data| {
                    score_screen_init(layout, None);
                })),
                "SkirmishMapSelectMenuInit" => Some(Box::new(|layout, data| {
                    skirmish_map_select_menu_init(layout, None);
                })),
                "SkirmishGameOptionsMenuInit" => Some(Box::new(|layout, data| {
                    skirmish_game_options_menu_init(layout, None);
                })),
                "LanMapSelectMenuInit" => Some(Box::new(|layout, data| {
                    lan_map_select_menu_init(layout, None);
                })),
                "LanGameOptionsMenuInit" => Some(Box::new(|layout, data| {
                    lan_game_options_menu_init(layout, None);
                })),
                "PopupHostGameInit" => Some(Box::new(|layout, data| {
                    popup_host_game_init(layout, data);
                })),
                "PopupLadderSelectInit" => Some(Box::new(|layout, data| {
                    popup_ladder_select_init(layout, data);
                })),
                "RCGameDetailsMenuInit" => Some(Box::new(|layout, data| {
                    rc_game_details_menu_init(layout, data);
                })),
                "DownloadMenuInit" => Some(Box::new(|layout, data| {
                    download_menu_init(layout, None);
                })),
                "GameInfoWindowInit" => Some(Box::new(|layout, data| {
                    game_info_window_init(layout, None);
                })),
                "NetworkDirectConnectInit" => Some(Box::new(|layout, data| {
                    network_direct_connect_init(layout, data);
                })),
                "WOLLoginMenuInit" => Some(Box::new(|layout, data| {
                    wol_login_menu_init(layout, data);
                })),
                "WOLLocaleSelectInit" => Some(Box::new(|layout, data| {
                    wol_locale_select_init(layout, data);
                })),
                "WOLMessageWindowInit" => Some(Box::new(|layout, data| {
                    wol_message_window_init(layout, data);
                })),
                "WOLBuddyOverlayInit" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_init(layout, data);
                })),
                "WOLBuddyOverlayRCMenuInit" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_rc_menu_init(layout, data);
                })),
                "WOLStatusMenuInit" => Some(Box::new(|layout, data| {
                    wol_status_menu_init(layout, data);
                })),
                "WOLWelcomeMenuInit" => Some(Box::new(|layout, data| {
                    wol_welcome_menu_init(layout, data);
                })),
                "WOLLobbyMenuInit" => Some(Box::new(|layout, data| {
                    wol_lobby_menu_init(layout, data);
                })),
                "WOLLadderScreenInit" => Some(Box::new(|layout, data| {
                    wol_ladder_screen_init(layout, data);
                })),
                "WOLMapSelectMenuInit" => Some(Box::new(|layout, data| {
                    wol_map_select_menu_init(layout, data);
                })),
                "WOLGameSetupMenuInit" => Some(Box::new(|layout, data| {
                    wol_game_setup_menu_init(layout, data);
                })),
                "WOLQuickMatchMenuInit" => Some(Box::new(|layout, data| {
                    wol_quick_match_menu_init(layout, data);
                })),
                "WOLQMScoreScreenInit" => Some(Box::new(|layout, data| {
                    wol_qm_score_screen_init(layout, data);
                })),
                "WOLCustomScoreScreenInit" => Some(Box::new(|layout, data| {
                    wol_custom_score_screen_init(layout, data);
                })),
                "MarketingScreenInit" => Some(Box::new(|_, _| {})),
                other => {
                    warn!("Unknown layout init callback: {}", other);
                    None
                }
            };
        }

        if !layout_def.update_callback.is_empty() {
            layout.update_callback = match layout_def.update_callback.as_str() {
                "MainMenuUpdate" => Some(Box::new(|layout, _| {
                    let mut menu = get_main_menu();
                    if let Err(err) = menu.update(layout, None) {
                        warn!("Main menu update failed: {}", err);
                    }
                })),
                "SinglePlayerMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_single_player_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Single player menu update failed: {}", err);
                    }
                })),
                "OptionsMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_options_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Options menu update failed: {}", err);
                    }
                })),
                "MapSelectMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_map_select_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Map select menu update failed: {}", err);
                    }
                })),
                "CreditsMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_credits_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Credits menu update failed: {}", err);
                    }
                })),
                "LanLobbyMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_lan_lobby_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("LAN lobby menu update failed: {}", err);
                    }
                })),
                "PopupCommunicatorUpdate" => Some(Box::new(|layout, _| {
                    popup_communicator_update(layout, None);
                })),
                "SaveLoadMenuUpdate" => Some(Box::new(|layout, data| {
                    save_load_menu_update(layout, data);
                })),
                "PopupReplayUpdate" => Some(Box::new(|layout, data| {
                    popup_replay_update(layout, data);
                })),
                "ReplayMenuUpdate" => Some(Box::new(|layout, data| {
                    replay_menu_update(layout, data);
                })),
                "ChallengeMenuUpdate" => Some(Box::new(|layout, data| {
                    challenge_menu_update(layout, data);
                })),
                "KeyboardOptionsMenuUpdate" => Some(Box::new(|layout, data| {
                    keyboard_options_menu_update(layout, data);
                })),
                "GameSpyPlayerInfoOverlayUpdate" => Some(Box::new(|layout, data| {
                    popup_player_info_update(layout, data);
                })),
                "PopupHostGameUpdate" => Some(Box::new(|layout, data| {
                    popup_host_game_update(layout, data);
                })),
                "PopupLadderSelectUpdate" => Some(Box::new(|layout, data| {
                    popup_ladder_select_update(layout, data);
                })),
                "DownloadMenuUpdate" => Some(Box::new(|layout, data| {
                    download_menu_update(layout, None);
                })),
                "ScoreScreenUpdate" => Some(Box::new(|layout, data| {
                    score_screen_update(layout, None);
                })),
                "SkirmishMapSelectMenuUpdate" => Some(Box::new(|layout, data| {
                    skirmish_map_select_menu_update(layout, None);
                })),
                "SkirmishGameOptionsMenuUpdate" => Some(Box::new(|layout, data| {
                    skirmish_game_options_menu_update(layout, None);
                })),
                "LanMapSelectMenuUpdate" => Some(Box::new(|layout, data| {
                    lan_map_select_menu_update(layout, None);
                })),
                "LanGameOptionsMenuUpdate" => Some(Box::new(|layout, data| {
                    lan_game_options_menu_update(layout, None);
                })),
                "NetworkDirectConnectUpdate" => Some(Box::new(|layout, data| {
                    network_direct_connect_update(layout, data);
                })),
                "WOLLoginMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_login_menu_update(layout, data);
                })),
                "WOLLocaleSelectUpdate" => Some(Box::new(|layout, data| {
                    wol_locale_select_update(layout, data);
                })),
                "WOLMessageWindowUpdate" => Some(Box::new(|layout, data| {
                    wol_message_window_update(layout, data);
                })),
                "WOLBuddyOverlayUpdate" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_update(layout, data);
                })),
                "WOLStatusMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_status_menu_update(layout, data);
                })),
                "WOLWelcomeMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_welcome_menu_update(layout, data);
                })),
                "WOLLobbyMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_lobby_menu_update(layout, data);
                })),
                "WOLLadderScreenUpdate" => Some(Box::new(|layout, data| {
                    wol_ladder_screen_update(layout, data);
                })),
                "WOLMapSelectMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_map_select_menu_update(layout, data);
                })),
                "WOLGameSetupMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_game_setup_menu_update(layout, data);
                })),
                "WOLQuickMatchMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_quick_match_menu_update(layout, data);
                })),
                "WOLQMScoreScreenUpdate" => Some(Box::new(|layout, data| {
                    wol_qm_score_screen_update(layout, data);
                })),
                "WOLCustomScoreScreenUpdate" => Some(Box::new(|layout, data| {
                    wol_custom_score_screen_update(layout, data);
                })),
                "MarketingScreenUpdate" => Some(Box::new(|_, _| {})),
                other => {
                    warn!("Unknown layout update callback: {}", other);
                    None
                }
            };
        }

        if !layout_def.shutdown_callback.is_empty() {
            layout.shutdown_callback = match layout_def.shutdown_callback.as_str() {
                "MainMenuShutdown" => Some(Box::new(|layout, data| {
                    let mut menu = get_main_menu();
                    if let Err(err) = menu.shutdown(layout, as_any_ref(data)) {
                        warn!("Main menu shutdown failed: {}", err);
                    }
                })),
                "SinglePlayerMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_single_player_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Single player menu shutdown failed: {}", err);
                    }
                })),
                "OptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_options_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Options menu shutdown failed: {}", err);
                    }
                })),
                "MapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_map_select_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Map select menu shutdown failed: {}", err);
                    }
                })),
                "CreditsMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_credits_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Credits menu shutdown failed: {}", err);
                    }
                })),
                "LanLobbyMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_lan_lobby_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("LAN lobby menu shutdown failed: {}", err);
                    }
                })),
                "PopupCommunicatorShutdown" => Some(Box::new(|layout, data| {
                    popup_communicator_shutdown(layout, as_any_ref(data));
                })),
                "SaveLoadMenuShutdown" => Some(Box::new(|layout, data| {
                    save_load_menu_shutdown(layout, as_any_ref(data));
                })),
                "PopupReplayShutdown" => Some(Box::new(|layout, data| {
                    popup_replay_shutdown(layout, as_any_ref(data));
                })),
                "ReplayMenuShutdown" => Some(Box::new(|layout, data| {
                    replay_menu_shutdown(layout, as_any_ref(data));
                })),
                "ChallengeMenuShutdown" => Some(Box::new(|layout, data| {
                    challenge_menu_shutdown(layout, as_any_ref(data));
                })),
                "KeyboardOptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    keyboard_options_menu_shutdown(layout, as_any_ref(data));
                })),
                "PopupLadderSelectShutdown" => Some(Box::new(|layout, data| {
                    popup_ladder_select_shutdown(layout, as_any_ref(data));
                })),
                "GameSpyPlayerInfoOverlayShutdown" => Some(Box::new(|layout, data| {
                    popup_player_info_shutdown(layout, as_any_ref(data));
                })),
                "DownloadMenuShutdown" => Some(Box::new(|layout, data| {
                    download_menu_shutdown(layout, data);
                })),
                "ScoreScreenShutdown" => Some(Box::new(|layout, data| {
                    score_screen_shutdown(layout, data);
                })),
                "SkirmishMapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    skirmish_map_select_menu_shutdown(layout, data);
                })),
                "SkirmishGameOptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    skirmish_game_options_menu_shutdown(layout, data);
                })),
                "LanMapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    lan_map_select_menu_shutdown(layout, data);
                })),
                "LanGameOptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    lan_game_options_menu_shutdown(layout, data);
                })),
                "NetworkDirectConnectShutdown" => Some(Box::new(|layout, data| {
                    network_direct_connect_shutdown(layout, as_any_ref(data));
                })),
                "WOLLoginMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_login_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLLocaleSelectShutdown" => Some(Box::new(|layout, data| {
                    wol_locale_select_shutdown(layout, as_any_ref(data));
                })),
                "WOLMessageWindowShutdown" => Some(Box::new(|layout, data| {
                    wol_message_window_shutdown(layout, as_any_ref(data));
                })),
                "WOLBuddyOverlayShutdown" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_shutdown(layout, as_any_ref(data));
                })),
                "WOLStatusMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_status_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLWelcomeMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_welcome_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLLobbyMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_lobby_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLLadderScreenShutdown" => Some(Box::new(|layout, data| {
                    wol_ladder_screen_shutdown(layout, as_any_ref(data));
                })),
                "WOLMapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_map_select_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLGameSetupMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_game_setup_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLQuickMatchMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_quick_match_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLQMScoreScreenShutdown" => Some(Box::new(|layout, data| {
                    wol_qm_score_screen_shutdown(layout, as_any_ref(data));
                })),
                "WOLCustomScoreScreenShutdown" => Some(Box::new(|layout, data| {
                    wol_custom_score_screen_shutdown(layout, as_any_ref(data));
                })),
                "ChallengeLoadScreenShutdown"
                | "MarketingScreenShutdown"
                | "SinglePlayerLoadScreenShutdown" => Some(Box::new(|_, _| {})),
                other => {
                    warn!("Unknown layout shutdown callback: {}", other);
                    None
                }
            };
        }
    }

    pub(crate) fn bind_window_callbacks(
        &self,
        window: &mut GameWindow,
        window_def: &WindowDefinition,
    ) {
        // C++ parseSystemCallback: TheFunctionLexicon->gameWinSystemFunc(nameToKey(str)).
        // "[None]" is not in the lexicon → NULL → createWindow skips winSetSystemFunc
        // and gadgets keep GadgetPushButtonSystem from gogoGadget*.
        if !is_none_callback_name(&window_def.system_callback) {
            let name = window_def.system_callback.as_str();
            match name {
                "GameWinDefaultSystem" => {
                    window.set_system_callback(default_system_callback);
                }
                "GadgetCheckBoxSystem"
                | "GadgetComboBoxSystem"
                | "GadgetHorizontalSliderSystem"
                | "GadgetListBoxSystem"
                | "GadgetProgressBarSystem"
                | "GadgetPushButtonSystem"
                | "GadgetRadioButtonSystem"
                | "GadgetStaticTextSystem"
                | "GadgetTabControlSystem"
                | "GadgetTextEntrySystem"
                | "GadgetVerticalSliderSystem"
                | "MOTDSystem" => {
                    window.set_system_callback(default_system_callback);
                }
                "ControlBarSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "ControlBarObserverSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_observer();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "DiplomacySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_diplomacy_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "InGameChatSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let chat = system.get_chat();
                        with_arc_write(&chat, |chat| chat.system(window, msg, data1, data2))
                    });
                }
                "ReplayControlSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let replay = system.get_replay();
                        with_arc_write(&replay, |replay| replay.system(window, msg, data1, data2))
                    });
                }
                "IdleWorkerSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let idle_worker = system.get_idle_worker();
                        with_arc_write(&idle_worker, |idle_worker| {
                            idle_worker.system(window, msg, data1, data2)
                        })
                    });
                }
                "MessageBoxSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_message_box_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let standard = system.get_standard();
                        with_arc_write(&standard, |standard| {
                            standard.system(window, msg, data1, data2)
                        })
                    });
                }
                "ExtendedMessageBoxSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_message_box_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let extended = system.get_extended();
                        with_arc_write(&extended, |extended| {
                            extended.system(window, msg, data1, data2)
                        })
                    });
                }
                "EstablishConnectionsControlSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let menu = get_establish_connections_menu();
                        let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
                        menu.system(window, msg, data1, data2)
                    });
                }
                "DisconnectControlSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let menu = get_disconnect_menu();
                        let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
                        menu.system(window, msg, data1, data2)
                    });
                }
                "MainMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let mut menu = get_main_menu();
                        let raw_msg = map_window_message_to_main_menu(msg);
                        if raw_msg == 0 {
                            return WindowMsgHandled::Ignored;
                        }
                        if menu.system(window.get_id() as u32, raw_msg, data1, data2) {
                            WindowMsgHandled::Handled
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                "SinglePlayerMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_single_player_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "OptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_options_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "MapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_map_select_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "CreditsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_credits_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "LanLobbyMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_lan_lobby_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "QuitMessageBoxSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_message_box_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let quit = system.get_quit();
                        with_arc_write(&quit, |quit| quit.system(window, msg, data1, data2))
                    });
                }
                "GeneralsExpPointsSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        generals_exp_points_system(window, msg, data1, data2)
                    });
                }
                "IMECandidateWindowSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        ime_candidate_window_system(window, msg, data1, data2)
                    });
                }
                "InGamePopupMessageSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        in_game_popup_message_system(window, msg, data1, data2)
                    });
                }
                "PopupCommunicatorSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_communicator_system(window, msg, data1, data2)
                    });
                }
                "PopupJoinGameSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_join_game_system(window, msg, data1, data2)
                    });
                }
                "PopupHostGameSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_host_game_system(window, msg, data1, data2)
                    });
                }
                "PopupLadderSelectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_ladder_select_system(window, msg, data1, data2)
                    });
                }
                "RCGameDetailsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        rc_game_details_menu_system(window, msg, data1, data2)
                    });
                }
                "DownloadMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        download_menu_system(window, msg, data1, data2)
                    });
                }
                "QuitMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        quit_menu_system(window, msg, data1, data2)
                    });
                }
                "SaveLoadMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        save_load_menu_system(window, msg, data1, data2)
                    });
                }
                "PopupReplaySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_replay_system(window, msg, data1, data2)
                    });
                }
                "ReplayMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        replay_menu_system(window, msg, data1, data2)
                    });
                }
                "ChallengeMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        challenge_menu_system(window, msg, data1, data2)
                    });
                }
                "DifficultySelectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        difficulty_select_system(window, msg, data1, data2)
                    });
                }
                "KeyboardOptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        keyboard_options_menu_system(window, msg, data1, data2)
                    });
                }
                "GameSpyPlayerInfoOverlaySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_player_info_system(window, msg, data1, data2)
                    });
                }
                "ScoreScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        score_screen_system(window, msg, data1, data2)
                    });
                }
                "SkirmishMapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        skirmish_map_select_menu_system(window, msg, data1, data2)
                    });
                }
                "SkirmishGameOptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        skirmish_game_options_menu_system(window, msg, data1, data2)
                    });
                }
                "LanMapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        lan_map_select_menu_system(window, msg, data1, data2)
                    });
                }
                "LanGameOptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        lan_game_options_menu_system(window, msg, data1, data2)
                    });
                }
                "GameInfoWindowSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        game_info_window_system(window, msg, data1, data2)
                    });
                }
                "NetworkDirectConnectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        network_direct_connect_system(window, msg, data1, data2)
                    });
                }
                "WOLLoginMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_login_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLLocaleSelectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_locale_select_system(window, msg, data1, data2)
                    });
                }
                "WOLMessageWindowSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_message_window_system(window, msg, data1, data2)
                    });
                }
                "WOLBuddyOverlaySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_buddy_overlay_system(window, msg, data1, data2)
                    });
                }
                "WOLBuddyOverlayRCMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_buddy_overlay_rc_menu_system(window, msg, data1, data2)
                    });
                }
                "PopupBuddyNotificationSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_buddy_notification_system(window, msg, data1, data2)
                    });
                }
                "WOLStatusMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_status_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLWelcomeMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_welcome_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLLobbyMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_lobby_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLLadderScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_ladder_screen_system(window, msg, data1, data2)
                    });
                }
                "WOLMapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_map_select_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLGameSetupMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_game_setup_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLQuickMatchMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_quick_match_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLQMScoreScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_qm_score_screen_system(window, msg, data1, data2)
                    });
                }
                "WOLCustomScoreScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_custom_score_screen_system(window, msg, data1, data2)
                    });
                }
                "PassMessagesToParentSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        if msg == WindowMessage::Create
                            || msg == WindowMessage::Destroy
                            || msg == WindowMessage::ScriptCreate
                        {
                            return WindowMsgHandled::Ignored;
                        }

                        if let Some(parent) = window.get_parent() {
                            if let Ok(mut parent_ref) = parent.try_borrow_mut() {
                                parent_ref.send_system_message(msg, data1, data2)
                            } else {
                                // Parent already borrowed. Fail-closed instead of
                                // aliasing the RefCell.
                                WindowMsgHandled::Ignored
                            }
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                "PassSelectedButtonsToParentSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        if msg != WindowMessage::GadgetSelected
                            && msg != WindowMessage::GadgetRightClick
                            && msg != WindowMessage::GadgetMouseEntering
                            && msg != WindowMessage::GadgetMouseLeaving
                            && msg != WindowMessage::GadgetEditDone
                        {
                            return WindowMsgHandled::Ignored;
                        }

                        if let Some(parent) = window.get_parent() {
                            if let Ok(mut parent_ref) = parent.try_borrow_mut() {
                                parent_ref.send_system_message(msg, data1, data2)
                            } else {
                                // Parent already borrowed. Fail-closed instead of
                                // aliasing the RefCell.
                                WindowMsgHandled::Ignored
                            }
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                other => {
                    warn!("Unimplemented system callback '{}', using default.", other);
                    window.set_system_callback(default_system_callback);
                }
            }
        }

        if !is_none_callback_name(&window_def.input_callback) {
            let name = window_def.input_callback.as_str();
            match name {
                "GameWinDefaultInput" => {
                    window.set_input_callback(default_input_callback);
                }
                "BeaconWindowInput" => {
                    window.set_input_callback(beacon_window_input);
                }
                "DisconnectControlInput"
                | "EstablishConnectionsControlInput"
                | "GadgetCheckBoxInput"
                | "GadgetComboBoxInput"
                | "GadgetHorizontalSliderInput"
                | "GadgetListBoxInput"
                | "GadgetListBoxMultiInput"
                | "GadgetPushButtonInput"
                | "GadgetRadioButtonInput"
                | "GadgetStaticTextInput"
                | "GadgetTabControlInput"
                | "GadgetTextEntryInput"
                | "GadgetVerticalSliderInput" => {
                    window.set_input_callback(default_input_callback);
                }
                "GameWinBlockInput" => {
                    window.set_input_callback(|_window, msg, _data1, _data2| match msg {
                        WindowMessage::Char | WindowMessage::MousePos => WindowMsgHandled::Ignored,
                        _ => WindowMsgHandled::Handled,
                    });
                }
                "ControlBarInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "LeftHUDInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_left_hud();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.input(window, msg, data1, data2)
                        })
                    });
                }
                "DiplomacyInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_diplomacy_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.input(window, msg, data1, data2)
                        })
                    });
                }
                "InGameChatInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let chat = system.get_chat();
                        with_arc_write(&chat, |chat| chat.input(window, msg, data1, data2))
                    });
                }
                "ReplayControlInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let replay = system.get_replay();
                        with_arc_write(&replay, |replay| replay.input(window, msg, data1, data2))
                    });
                }
                "MainMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let mut menu = get_main_menu();
                        let raw_msg = map_window_message_to_main_menu(msg);
                        if raw_msg == 0 {
                            return WindowMsgHandled::Ignored;
                        }
                        if menu.input(window.get_id() as u32, raw_msg, data1, data2) {
                            WindowMsgHandled::Handled
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                "SinglePlayerMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_single_player_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "OptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_options_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "MapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_map_select_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "CreditsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_credits_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "LanLobbyMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_lan_lobby_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "GeneralsExpPointsInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        generals_exp_points_input(window, msg, data1, data2)
                    });
                }
                "IMECandidateWindowInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        ime_candidate_window_input(window, msg, data1, data2)
                    });
                }
                "InGamePopupMessageInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        in_game_popup_message_input(window, msg, data1, data2)
                    });
                }
                "PopupCommunicatorInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_communicator_input(window, msg, data1, data2)
                    });
                }
                "PopupJoinGameInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_join_game_input(window, msg, data1, data2)
                    });
                }
                "PopupHostGameInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_host_game_input(window, msg, data1, data2)
                    });
                }
                "PopupLadderSelectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_ladder_select_input(window, msg, data1, data2)
                    });
                }
                "DownloadMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        download_menu_input(window, msg, data1, data2)
                    });
                }
                "SaveLoadMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        save_load_menu_input(window, msg, data1, data2)
                    });
                }
                "PopupReplayInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_replay_input(window, msg, data1, data2)
                    });
                }
                "ReplayMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        replay_menu_input(window, msg, data1, data2)
                    });
                }
                "ChallengeMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        challenge_menu_input(window, msg, data1, data2)
                    });
                }
                "DifficultySelectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        difficulty_select_input(window, msg, data1, data2)
                    });
                }
                "KeyboardOptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        keyboard_options_menu_input(window, msg, data1, data2)
                    });
                }
                "GameSpyPlayerInfoOverlayInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_player_info_input(window, msg, data1, data2)
                    });
                }
                "ScoreScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        score_screen_input(window, msg, data1, data2)
                    });
                }
                "SkirmishMapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        skirmish_map_select_menu_input(window, msg, data1, data2)
                    });
                }
                "SkirmishGameOptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        skirmish_game_options_menu_input(window, msg, data1, data2)
                    });
                }
                "LanMapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        lan_map_select_menu_input(window, msg, data1, data2)
                    });
                }
                "LanGameOptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        lan_game_options_menu_input(window, msg, data1, data2)
                    });
                }
                "NetworkDirectConnectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        network_direct_connect_input(window, msg, data1, data2)
                    });
                }
                "WOLLoginMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_login_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLLocaleSelectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_locale_select_input(window, msg, data1, data2)
                    });
                }
                "WOLMessageWindowInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_message_window_input(window, msg, data1, data2)
                    });
                }
                "WOLBuddyOverlayInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_buddy_overlay_input(window, msg, data1, data2)
                    });
                }
                "WOLStatusMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_status_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLWelcomeMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_welcome_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLLobbyMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_lobby_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLLadderScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_ladder_screen_input(window, msg, data1, data2)
                    });
                }
                "WOLMapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_map_select_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLGameSetupMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_game_setup_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLQuickMatchMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_quick_match_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLQMScoreScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_qm_score_screen_input(window, msg, data1, data2)
                    });
                }
                "WOLCustomScoreScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_custom_score_screen_input(window, msg, data1, data2)
                    });
                }
                other => {
                    warn!("Unimplemented input callback '{}', using default.", other);
                    window.set_input_callback(default_input_callback);
                }
            }
        }

        if !is_none_callback_name(&window_def.tooltip_callback) {
            let name = window_def.tooltip_callback.as_str();
            match name {
                "GameWinDefaultTooltip" => {
                    window.set_tooltip_callback(default_tooltip_callback);
                }
                other => {
                    warn!("Unimplemented tooltip callback '{}', using default.", other);
                    window.set_tooltip_callback(default_tooltip_callback);
                }
            }
        }

        if !is_none_callback_name(&window_def.draw_callback) {
            let name = window_def.draw_callback.as_str();
            match name {
                "GameWinDefaultDraw" => {
                    // C++ FunctionLexicon: GameWinDefaultDraw is a no-op.
                    window.set_draw_callback(legacy_default_draw_callback);
                }
                "W3DGameWinDefaultDraw" => {
                    window.set_draw_callback(default_draw_callback);
                }
                "W3DGadgetPushButtonDraw" => {
                    window.set_draw_callback(w3d_gadget_push_button_draw);
                }
                "W3DGadgetPushButtonImageDraw" => {
                    window.set_draw_callback(w3d_gadget_push_button_image_draw);
                }
                "W3DGadgetStaticTextDraw" => {
                    window.set_draw_callback(w3d_gadget_static_text_draw);
                }
                "W3DGadgetStaticTextImageDraw" => {
                    window.set_draw_callback(w3d_gadget_static_text_image_draw);
                }
                "W3DGadgetProgressBarDraw" => {
                    window.set_draw_callback(w3d_gadget_progress_bar_draw);
                }
                "W3DGadgetProgressBarImageDraw" => {
                    window.set_draw_callback(w3d_gadget_progress_bar_image_draw);
                }
                "W3DGadgetProgressBarImageDrawA" => {
                    window.set_draw_callback(w3d_gadget_progress_bar_image_draw_a);
                }
                "W3DGadgetCheckBoxDraw" => {
                    window.set_draw_callback(w3d_gadget_check_box_draw);
                }
                "W3DGadgetCheckBoxImageDraw" => {
                    window.set_draw_callback(w3d_gadget_check_box_image_draw);
                }
                "W3DGadgetRadioButtonDraw" => {
                    window.set_draw_callback(w3d_gadget_radio_button_draw);
                }
                "W3DGadgetRadioButtonImageDraw" => {
                    window.set_draw_callback(w3d_gadget_radio_button_image_draw);
                }
                "W3DGadgetHorizontalSliderDraw" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_draw);
                }
                "W3DGadgetHorizontalSliderImageDraw" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_image_draw);
                }
                "W3DGadgetHorizontalSliderImageDrawA" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_image_draw_a);
                }
                "W3DGadgetHorizontalSliderImageDrawB" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_image_draw_b);
                }
                "W3DGadgetVerticalSliderDraw" => {
                    window.set_draw_callback(w3d_gadget_vertical_slider_draw);
                }
                "W3DGadgetVerticalSliderImageDraw" => {
                    window.set_draw_callback(w3d_gadget_vertical_slider_image_draw);
                }
                "W3DGadgetTextEntryDraw" => {
                    window.set_draw_callback(w3d_gadget_text_entry_draw);
                }
                "W3DGadgetTextEntryImageDraw" => {
                    window.set_draw_callback(w3d_gadget_text_entry_image_draw);
                }
                "W3DGadgetListBoxDraw" => {
                    window.set_draw_callback(w3d_gadget_list_box_draw);
                }
                "W3DGadgetListBoxImageDraw" => {
                    window.set_draw_callback(w3d_gadget_list_box_image_draw);
                }
                "W3DGadgetTabControlDraw" => {
                    window.set_draw_callback(w3d_gadget_tab_control_draw);
                }
                "W3DGadgetTabControlImageDraw" => {
                    window.set_draw_callback(w3d_gadget_tab_control_image_draw);
                }
                "W3DGadgetComboBoxDraw" => {
                    window.set_draw_callback(w3d_gadget_combo_box_draw);
                }
                "W3DGadgetComboBoxImageDraw" => {
                    window.set_draw_callback(w3d_gadget_combo_box_image_draw);
                }
                "W3DMainMenuDraw" => {
                    window.set_draw_callback(w3d_main_menu_draw);
                }
                "W3DMainMenuFourDraw" => {
                    window.set_draw_callback(w3d_main_menu_four_draw);
                }
                "W3DMetalBarMenuDraw" => {
                    window.set_draw_callback(w3d_metal_bar_menu_draw);
                }
                "W3DCreditsMenuDraw" => {
                    window.set_draw_callback(w3d_credits_menu_draw);
                }
                "W3DShellMenuSchemeDraw" => {
                    window.set_draw_callback(w3d_shell_menu_scheme_draw);
                }
                "W3DClockDraw" => {
                    window.set_draw_callback(w3d_clock_draw);
                }
                "W3DMainMenuMapBorder" => {
                    window.set_draw_callback(w3d_main_menu_map_border);
                }
                "W3DMainMenuButtonDropShadowDraw" => {
                    window.set_draw_callback(w3d_main_menu_button_drop_shadow_draw);
                }
                "W3DMainMenuRandomTextDraw" => {
                    window.set_draw_callback(w3d_main_menu_random_text_draw);
                }
                "W3DThinBorderDraw" => {
                    window.set_draw_callback(w3d_thin_border_draw);
                }
                "W3DCameoMovieDraw" => {
                    window.set_draw_callback(w3d_cameo_movie_draw);
                }
                "W3DLeftHUDDraw" => {
                    window.set_draw_callback(w3d_left_hud_draw);
                }
                "W3DRightHUDDraw" => {
                    window.set_draw_callback(w3d_right_hud_draw);
                }
                "W3DPowerDraw" => {
                    window.set_draw_callback(w3d_power_draw);
                }
                "W3DPowerDrawA" => {
                    window.set_draw_callback(w3d_power_draw_a);
                }
                "W3DCommandBarTopDraw" => {
                    window.set_draw_callback(w3d_command_bar_top_draw);
                }
                "W3DCommandBarBackgroundDraw" => {
                    window.set_draw_callback(w3d_command_bar_background_draw);
                }
                "W3DCommandBarForegroundDraw" => {
                    window.set_draw_callback(w3d_command_bar_foreground_draw);
                }
                "W3DCommandBarGridDraw" => {
                    window.set_draw_callback(w3d_command_bar_grid_draw);
                }
                "W3DCommandBarGenExpDraw" => {
                    window.set_draw_callback(w3d_command_bar_gen_exp_draw);
                }
                "W3DCommandBarHelpPopupDraw" => {
                    window.set_draw_callback(w3d_command_bar_help_popup_draw);
                }
                "W3DNoDraw" => {
                    window.set_draw_callback(w3d_no_draw);
                }
                "W3DDrawMapPreview" => {
                    window.set_draw_callback(w3d_draw_map_preview);
                }
                "IMECandidateMainDraw" => {
                    window.set_draw_callback(ime_candidate_main_draw);
                }
                "IMECandidateTextAreaDraw" => {
                    window.set_draw_callback(|window, inst| {
                        ime_candidate_text_area_draw(window, inst)
                    });
                }
                other => {
                    warn!("Unimplemented draw callback '{}', using default.", other);
                    window.set_draw_callback(default_draw_callback);
                }
            }
        }

        if let Some(edit_data) = window.get_edit_data_mut() {
            edit_data.system_callback_string = window_def.system_callback.clone();
            edit_data.input_callback_string = window_def.input_callback.clone();
            edit_data.tooltip_callback_string = window_def.tooltip_callback.clone();
            edit_data.draw_callback_string = window_def.draw_callback.clone();
        }
    }
}
