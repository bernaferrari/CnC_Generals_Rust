//! Layout and per-window script callback binding (FunctionLexicon parity).
#![allow(unused_imports)]

use crate::gui::callbacks::menu_callbacks::MenuCallbacks;
use crate::gui::callbacks::{
    beacon_window_input, challenge_menu_init, challenge_menu_input, challenge_menu_shutdown,
    challenge_menu_system, challenge_menu_update, difficulty_select_init, difficulty_select_input,
    difficulty_select_system, download_menu_init, download_menu_input, download_menu_shutdown,
    download_menu_system, download_menu_update, game_info_window_init, game_info_window_system,
    generals_exp_points_input, generals_exp_points_system, get_control_bar_system,
    get_diplomacy_system, get_ingame_ui_system, get_message_box_system, ime_candidate_main_draw,
    ime_candidate_text_area_draw, ime_candidate_window_input, ime_candidate_window_system,
    in_game_popup_message_init, in_game_popup_message_input,
    in_game_popup_message_system_with_sink, keyboard_options_menu_init,
    keyboard_options_menu_input, keyboard_options_menu_shutdown, keyboard_options_menu_system,
    keyboard_options_menu_update, lan_game_options_menu_init, lan_game_options_menu_input,
    lan_game_options_menu_shutdown, lan_game_options_menu_system, lan_game_options_menu_update,
    lan_map_select_menu_init, lan_map_select_menu_input, lan_map_select_menu_shutdown,
    lan_map_select_menu_system, lan_map_select_menu_update, network_direct_connect_init,
    network_direct_connect_input, network_direct_connect_shutdown, network_direct_connect_system,
    network_direct_connect_update, popup_buddy_notification_system, popup_communicator_init,
    popup_communicator_input, popup_communicator_shutdown, popup_communicator_system,
    popup_communicator_update, popup_host_game_init, popup_host_game_input, popup_host_game_system,
    popup_host_game_update, popup_join_game_init, popup_join_game_input, popup_join_game_system,
    popup_ladder_select_init, popup_ladder_select_input, popup_ladder_select_shutdown,
    popup_ladder_select_system, popup_ladder_select_update, popup_player_info_init,
    popup_player_info_input, popup_player_info_shutdown, popup_player_info_system,
    popup_player_info_update, popup_replay_init, popup_replay_input, popup_replay_shutdown,
    popup_replay_system, popup_replay_update, quit_menu_system, rc_game_details_menu_init,
    rc_game_details_menu_system, replay_menu_init, replay_menu_input, replay_menu_shutdown,
    replay_menu_system, replay_menu_update, save_load_menu_full_screen_init, save_load_menu_init,
    save_load_menu_input, save_load_menu_shutdown, save_load_menu_system, save_load_menu_update,
    score_screen_init, score_screen_input, score_screen_shutdown, score_screen_system,
    score_screen_update, skirmish_game_options_menu_init, skirmish_game_options_menu_input,
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
use crate::helpers::TheControlBar;
use game_engine::common::name_key_generator::NameKeyGenerator;
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
                "W3DMainMenuInit" | "MainMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        effects.with_manager(|manager, effects| {
                            apply_w3d_main_menu_runtime_draw_overrides(manager)
                        });
                        if let Err(err) =
                            effects.with_main_menu(|menu, effects| menu.init(layout, None, effects))
                        {
                            warn!("Main menu init failed: {}", err);
                        }
                    }));
                    None
                }
                "SinglePlayerMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects.with_single_player_menu(|menu, effects| {
                            menu.init_with_effects(layout, None, effects)
                        }) {
                            warn!("Single player menu init failed: {}", err);
                        }
                    }));
                    None
                }
                "OptionsMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        let camera=data.and_then(|data|data.downcast_ref::<crate::gui::callbacks::OptionsCameraInputObservation>());
                        if let Err(err) = effects.with_options_menu(|menu, effects| {
                            menu.init_with_camera_input(layout, camera, effects)
                        }) {
                            warn!("Options menu init failed: {}", err);
                        }
                    }));
                    None
                }
                "MapSelectMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects.with_map_select_menu(|menu, effects| {
                            menu.init_with_effects(layout, None, effects)
                        }) {
                            warn!("Map select menu init failed: {}", err);
                        }
                    }));
                    None
                }
                "CreditsMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        crate::gui::callbacks::owned_credits_menu::init(layout, effects)
                    }));
                    None
                }
                "LanLobbyMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects.with_lan_lobby_menu(|menu, effects| {
                            menu.init_with_effects(layout, None, effects)
                        }) {
                            warn!("LAN lobby menu init failed: {}", err);
                        }
                    }));
                    None
                }
                "InGamePopupMessageInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        in_game_popup_message_init(layout, None, effects);
                    }));
                    None
                }
                "PopupCommunicatorInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        effects.with_manager(|manager, effects| {
                            popup_communicator_init(layout, None, manager, effects)
                        });
                    }));
                    None
                }
                "PopupJoinGameInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        popup_join_game_init(layout, None);
                    }));
                    None
                }
                "SaveLoadMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        save_load_menu_init(layout, data, effects);
                    }));
                    None
                }
                "SaveLoadMenuFullScreenInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        save_load_menu_full_screen_init(layout, data, effects);
                    }));
                    None
                }
                "PopupReplayInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        effects.with_manager(|manager, effects| {
                            popup_replay_init(layout, data, manager, effects)
                        });
                    }));
                    None
                }
                "ReplayMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        replay_menu_init(layout, data, effects);
                    }));
                    None
                }
                "ChallengeMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        challenge_menu_init(layout, data, effects);
                    }));
                    None
                }
                "DifficultySelectInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        difficulty_select_init(layout, data, effects);
                    }));
                    None
                }
                "KeyboardOptionsMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        keyboard_options_menu_init(layout, data, effects);
                    }));
                    None
                }
                "GameSpyPlayerInfoOverlayInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        popup_player_info_init(layout, data);
                    }));
                    None
                }
                "ScoreScreenInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        score_screen_init(layout, None, effects);
                    }));
                    None
                }
                "SkirmishMapSelectMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        skirmish_map_select_menu_init(layout, None, effects);
                    }));
                    None
                }
                "SkirmishGameOptionsMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        skirmish_game_options_menu_init(layout, None, effects);
                    }));
                    None
                }
                "LanMapSelectMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        lan_map_select_menu_init(layout, None, effects);
                    }));
                    None
                }
                "LanGameOptionsMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        lan_game_options_menu_init(layout, None, effects);
                    }));
                    None
                }
                "PopupHostGameInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        popup_host_game_init(layout, data);
                    }));
                    None
                }
                "PopupLadderSelectInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        popup_ladder_select_init(layout, data);
                    }));
                    None
                }
                "RCGameDetailsMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        rc_game_details_menu_init(layout, data);
                    }));
                    None
                }
                "DownloadMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        effects.with_manager(|manager, effects| {
                            download_menu_init(layout, None, manager)
                        });
                    }));
                    None
                }
                "GameInfoWindowInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        effects.with_manager(|manager, effects| {
                            game_info_window_init(layout, None, manager)
                        });
                    }));
                    None
                }
                "NetworkDirectConnectInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        network_direct_connect_init(layout, data, effects);
                    }));
                    None
                }
                "WOLLoginMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_login_menu_init(layout, data);
                    }));
                    None
                }
                "WOLLocaleSelectInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_locale_select_init(layout, data);
                    }));
                    None
                }
                "WOLMessageWindowInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_message_window_init(layout, data);
                    }));
                    None
                }
                "WOLBuddyOverlayInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_buddy_overlay_init(layout, data);
                    }));
                    None
                }
                "WOLBuddyOverlayRCMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_buddy_overlay_rc_menu_init(layout, data);
                    }));
                    None
                }
                "WOLStatusMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_status_menu_init(layout, data);
                    }));
                    None
                }
                "WOLWelcomeMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_welcome_menu_init(layout, data);
                    }));
                    None
                }
                "WOLLobbyMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_lobby_menu_init(layout, data);
                    }));
                    None
                }
                "WOLLadderScreenInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_ladder_screen_init(layout, data);
                    }));
                    None
                }
                "WOLMapSelectMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_map_select_menu_init(layout, data);
                    }));
                    None
                }
                "WOLGameSetupMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_game_setup_menu_init(layout, data);
                    }));
                    None
                }
                "WOLQuickMatchMenuInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_quick_match_menu_init(layout, data);
                    }));
                    None
                }
                "WOLQMScoreScreenInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_qm_score_screen_init(layout, data);
                    }));
                    None
                }
                "WOLCustomScoreScreenInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|layout, data, effects| {
                        wol_custom_score_screen_init(layout, data);
                    }));
                    None
                }
                "MarketingScreenInit" => {
                    layout.init_callback_with_effects = Some(Box::new(|_, _, effects| {}));
                    None
                }
                other => {
                    warn!("Unknown layout init callback: {}", other);
                    None
                }
            };
        }

        if !layout_def.update_callback.is_empty() {
            layout.update_callback = match layout_def.update_callback.as_str() {
                "MainMenuUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects
                            .with_main_menu(|menu, effects| menu.update(layout, None, effects))
                        {
                            warn!("Main menu update failed: {}", err);
                        }
                    }));
                    None
                }
                "SinglePlayerMenuUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects.with_single_player_menu(|menu, effects| {
                            menu.update_with_effects(layout, None, effects)
                        }) {
                            warn!("Single player menu update failed: {}", err);
                        }
                    }));
                    None
                }
                "OptionsMenuUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects
                            .with_options_menu(|menu, effects| menu.update(layout, None, effects))
                        {
                            warn!("Options menu update failed: {}", err);
                        }
                    }));
                    None
                }
                "MapSelectMenuUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects.with_map_select_menu(|menu, effects| {
                            menu.update_with_effects(layout, None, effects)
                        }) {
                            warn!("Map select menu update failed: {}", err);
                        }
                    }));
                    None
                }
                "CreditsMenuUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        crate::gui::callbacks::owned_credits_menu::update(layout, effects)
                    }));
                    None
                }
                "LanLobbyMenuUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        if let Err(err) = effects
                            .with_lan_lobby_menu(|menu, effects| menu.update(layout, None, effects))
                        {
                            warn!("LAN lobby menu update failed: {}", err);
                        }
                    }));
                    None
                }
                "PopupCommunicatorUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        popup_communicator_update(layout, None);
                    }));
                    None
                }
                "SaveLoadMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            save_load_menu_update(layout, data, effects);
                        }));
                    None
                }
                "PopupReplayUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            effects.with_manager(|manager, effects| {
                                popup_replay_update(layout, data, manager)
                            });
                        }));
                    None
                }
                "ReplayMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            replay_menu_update(layout, data, effects);
                        }));
                    None
                }
                "ChallengeMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            challenge_menu_update(layout, data, effects)
                        }));
                    None
                }
                "KeyboardOptionsMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            keyboard_options_menu_update(layout, data);
                        }));
                    None
                }
                "GameSpyPlayerInfoOverlayUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            popup_player_info_update(layout, data);
                        }));
                    None
                }
                "PopupHostGameUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            popup_host_game_update(layout, data);
                        }));
                    None
                }
                "PopupLadderSelectUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            popup_ladder_select_update(layout, data);
                        }));
                    None
                }
                "DownloadMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            effects.with_manager(|manager, effects| {
                                download_menu_update(layout, None, manager, effects)
                            });
                        }));
                    None
                }
                "ScoreScreenUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            score_screen_update(layout, None, effects);
                        }));
                    None
                }
                "SkirmishMapSelectMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            skirmish_map_select_menu_update(layout, None);
                        }));
                    None
                }
                "SkirmishGameOptionsMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            skirmish_game_options_menu_update(layout, None, effects);
                        }));
                    None
                }
                "LanMapSelectMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            lan_map_select_menu_update(layout, None);
                        }));
                    None
                }
                "LanGameOptionsMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            lan_game_options_menu_update(layout, None, effects);
                        }));
                    None
                }
                "NetworkDirectConnectUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            network_direct_connect_update(layout, data);
                        }));
                    None
                }
                "WOLLoginMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_login_menu_update(layout, data);
                        }));
                    None
                }
                "WOLLocaleSelectUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_locale_select_update(layout, data);
                        }));
                    None
                }
                "WOLMessageWindowUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_message_window_update(layout, data);
                        }));
                    None
                }
                "WOLBuddyOverlayUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_buddy_overlay_update(layout, data);
                        }));
                    None
                }
                "WOLStatusMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_status_menu_update(layout, data);
                        }));
                    None
                }
                "WOLWelcomeMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_welcome_menu_update(layout, data);
                        }));
                    None
                }
                "WOLLobbyMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_lobby_menu_update(layout, data);
                        }));
                    None
                }
                "WOLLadderScreenUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_ladder_screen_update(layout, data);
                        }));
                    None
                }
                "WOLMapSelectMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_map_select_menu_update(layout, data);
                        }));
                    None
                }
                "WOLGameSetupMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_game_setup_menu_update(layout, data);
                        }));
                    None
                }
                "WOLQuickMatchMenuUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_quick_match_menu_update(layout, data);
                        }));
                    None
                }
                "WOLQMScoreScreenUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_qm_score_screen_update(layout, data);
                        }));
                    None
                }
                "WOLCustomScoreScreenUpdate" => {
                    layout.update_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_custom_score_screen_update(layout, data);
                        }));
                    None
                }
                "MarketingScreenUpdate" => {
                    layout.update_callback_with_effects = Some(Box::new(|_, _, effects| {}));
                    None
                }
                other => {
                    warn!("Unknown layout update callback: {}", other);
                    None
                }
            };
        }

        if !layout_def.shutdown_callback.is_empty() {
            layout.shutdown_callback = match layout_def.shutdown_callback.as_str() {
                "MainMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            if let Err(err) = effects.with_main_menu(|menu, effects| {
                                menu.shutdown(layout, as_any_ref(data), effects)
                            }) {
                                warn!("Main menu shutdown failed: {err}");
                            }
                        }));
                    None
                }
                "SinglePlayerMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            if let Err(err) = effects.with_single_player_menu(|menu, effects| {
                                menu.shutdown(layout, None, effects)
                            }) {
                                warn!("Single player menu shutdown failed: {}", err);
                            }
                        }));
                    None
                }
                "OptionsMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            if let Err(err) = effects.with_options_menu(|menu, effects| {
                                menu.shutdown(layout, None, effects)
                            }) {
                                warn!("Options menu shutdown failed: {}", err);
                            }
                        }));
                    None
                }
                "MapSelectMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            if let Err(err) = effects.with_map_select_menu(|menu, effects| {
                                menu.shutdown(layout, None, effects)
                            }) {
                                warn!("Map select menu shutdown failed: {}", err);
                            }
                        }));
                    None
                }
                "CreditsMenuShutdown" => {
                    layout.shutdown_callback_with_effects = Some(Box::new(|layout, _, effects| {
                        crate::gui::callbacks::owned_credits_menu::shutdown(layout, effects)
                    }));
                    None
                }
                "LanLobbyMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            if let Err(err) = effects.with_lan_lobby_menu(|menu, effects| {
                                menu.shutdown(layout, None, effects)
                            }) {
                                warn!("LAN lobby menu shutdown failed: {}", err);
                            }
                        }));
                    None
                }
                "PopupCommunicatorShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            popup_communicator_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "SaveLoadMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            save_load_menu_shutdown(layout, as_any_ref(data), effects);
                        }));
                    None
                }
                "PopupReplayShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            popup_replay_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "ReplayMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            replay_menu_shutdown(layout, as_any_ref(data), effects);
                        }));
                    None
                }
                "ChallengeMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            challenge_menu_shutdown(layout, as_any_ref(data), effects);
                        }));
                    None
                }
                "KeyboardOptionsMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            keyboard_options_menu_shutdown(layout, as_any_ref(data), effects);
                        }));
                    None
                }
                "PopupLadderSelectShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            popup_ladder_select_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "GameSpyPlayerInfoOverlayShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            popup_player_info_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "DownloadMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            download_menu_shutdown(layout, data);
                        }));
                    None
                }
                "ScoreScreenShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            score_screen_shutdown(layout, data, effects);
                        }));
                    None
                }
                "SkirmishMapSelectMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            skirmish_map_select_menu_shutdown(layout, data, effects);
                        }));
                    None
                }
                "SkirmishGameOptionsMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            skirmish_game_options_menu_shutdown(layout, data, effects);
                        }));
                    None
                }
                "LanMapSelectMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            lan_map_select_menu_shutdown(layout, data, effects);
                        }));
                    None
                }
                "LanGameOptionsMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            lan_game_options_menu_shutdown(layout, data, effects);
                        }));
                    None
                }
                "NetworkDirectConnectShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            network_direct_connect_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLLoginMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_login_menu_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLLocaleSelectShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_locale_select_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLMessageWindowShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_message_window_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLBuddyOverlayShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_buddy_overlay_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLStatusMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_status_menu_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLWelcomeMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_welcome_menu_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLLobbyMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_lobby_menu_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLLadderScreenShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_ladder_screen_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLMapSelectMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_map_select_menu_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLGameSetupMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_game_setup_menu_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLQuickMatchMenuShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_quick_match_menu_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLQMScoreScreenShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_qm_score_screen_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "WOLCustomScoreScreenShutdown" => {
                    layout.shutdown_callback_with_effects =
                        Some(Box::new(|layout, data, effects| {
                            wol_custom_score_screen_shutdown(layout, as_any_ref(data));
                        }));
                    None
                }
                "ChallengeLoadScreenShutdown"
                | "MarketingScreenShutdown"
                | "SinglePlayerLoadScreenShutdown" => {
                    layout.shutdown_callback_with_effects = Some(Box::new(|_, _, effects| {}));
                    None
                }
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
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_control_bar_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let callbacks = system.get_callbacks();
                            with_arc_write(&callbacks, |callbacks| {
                                callbacks.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                    window.mark_control_bar_system_callback();
                }
                "ControlBarObserverSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_control_bar_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let callbacks = system.get_observer();
                            with_arc_write(&callbacks, |callbacks| {
                                callbacks.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                    window.mark_control_bar_observer_system_callback();
                }
                "DiplomacySystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            // GWM_CREATE is dispatched synchronously by
                            // create_window_from_definition (layout_load.rs:266)
                            // while DiplomacySystem::toggle_diplomacy still holds
                            // the DiplomacyCallbacks write guard; re-acquiring it
                            // here self-deadlocks the whole client thread. The
                            // Create side effect (slot-table populate) runs in
                            // show_layout on every show — C++ populates in
                            // ShowDiplomacy via PopulateInGameDiplomacyPopup
                            // (Diplomacy.cpp:247-248) — so Create only needs to
                            // report handled.
                            if msg == WindowMessage::Create {
                                return WindowMsgHandled::Handled;
                            }
                            let system = get_diplomacy_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let callbacks = system.get_callbacks();
                            with_arc_write(&callbacks, |callbacks| {
                                callbacks.system(window, msg, data1, data2)
                            })
                        },
                    );
                }
                "InGameChatSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_ingame_ui_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let chat = system.get_chat();
                            with_arc_write(&chat, |chat| {
                                effects.with_manager(|manager, effects| {
                                    chat.system(window, msg, data1, data2, manager, effects)
                                })
                            })
                        },
                    );
                }
                "ReplayControlSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_ingame_ui_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let replay = system.get_replay();
                            with_arc_write(&replay, |replay| {
                                effects.with_manager(|manager, effects| {
                                    replay.system(window, msg, data1, data2, manager)
                                })
                            })
                        },
                    );
                }
                "IdleWorkerSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_ingame_ui_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let idle_worker = system.get_idle_worker();
                            with_arc_write(&idle_worker, |idle_worker| {
                                effects.with_manager(|manager, effects| {
                                    idle_worker.system(window, msg, data1, data2, manager)
                                })
                            })
                        },
                    );
                }
                "MessageBoxSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_message_box_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let standard = system.get_standard();
                            with_arc_write(&standard, |standard| {
                                standard.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "ExtendedMessageBoxSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_message_box_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let extended = system.get_extended();
                            with_arc_write(&extended, |extended| {
                                extended.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "EstablishConnectionsControlSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            if !matches!(msg, WindowMessage::GadgetSelected) {
                                return WindowMsgHandled::Ignored;
                            }
                            effects.with_establish_connections_menu(|menu, effects| {
                                menu.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "DisconnectControlSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            if !matches!(
                                msg,
                                WindowMessage::GadgetSelected | WindowMessage::GadgetEditDone
                            ) {
                                return WindowMsgHandled::Ignored;
                            }
                            effects.with_disconnect_menu(|menu, effects| {
                                menu.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "MainMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let raw_msg = map_window_message_to_main_menu(msg);
                            if raw_msg == 0 {
                                return WindowMsgHandled::Ignored;
                            }
                            if effects.with_main_menu_for_window(window, |menu, effects, window| {
                                menu.system_for_window(window, raw_msg, data1, data2, effects)
                            }) {
                                WindowMsgHandled::Handled
                            } else {
                                WindowMsgHandled::Ignored
                            }
                        },
                    );
                    window.mark_main_menu_system_callback();
                }
                "SinglePlayerMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_single_player_menu(|menu, effects| {
                                menu.system_with_effects(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "OptionsMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_options_menu(|menu, effects| {
                                menu.system_with_effects(window, msg, data1, data2, effects)
                            })
                        },
                    );
                    window.mark_options_menu_system_callback();
                }
                "MapSelectMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_map_select_menu(|menu, effects| {
                                menu.system_with_effects(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "CreditsMenuSystem" => {
                    window.set_system_callback(|window, msg, d1, d2| {
                        crate::gui::callbacks::owned_credits_menu::system(window, msg, d1, d2)
                    });
                }
                "LanLobbyMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_lan_lobby_menu(|menu, effects| {
                                menu.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "QuitMessageBoxSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_message_box_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let quit = system.get_quit();
                            with_arc_write(&quit, |quit| {
                                quit.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "GeneralsExpPointsSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            if msg == WindowMessage::GadgetSelected {
                                let control_id = data1 as u32;
                                let button_exit_id = NameKeyGenerator::name_to_key(
                                    "GeneralsExpPoints.wnd:ButtonExit",
                                );
                                if control_id == button_exit_id {
                                    TheControlBar::hide_purchase_science();
                                } else if let Some(clicks) =
                                    effects.pending_control_bar_clicks.as_deref_mut()
                                {
                                    const GBM_SELECTED: u32 = 16384 + 8;
                                    clicks.push(
                                        crate::helpers::LiveControlBarClick::from_current_dispatch(
                                            control_id,
                                            GBM_SELECTED,
                                        ),
                                    );
                                }
                                return WindowMsgHandled::Handled;
                            }
                            generals_exp_points_system(window, msg, data1, data2)
                        },
                    );
                }
                "IMECandidateWindowSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            ime_candidate_window_system(window, msg, data1, data2)
                        },
                    );
                }
                "InGamePopupMessageSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            in_game_popup_message_system_with_sink(
                                window, msg, data1, data2, effects,
                            )
                        },
                    );
                }
                "PopupCommunicatorSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_manager(|manager, effects| {
                                popup_communicator_system(
                                    window, msg, data1, data2, manager, effects,
                                )
                            })
                        },
                    );
                }
                "PopupJoinGameSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_join_game_system(window, msg, data1, data2)
                        },
                    );
                }
                "PopupHostGameSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_host_game_system(window, msg, data1, data2)
                        },
                    );
                }
                "PopupLadderSelectSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_ladder_select_system(window, msg, data1, data2)
                        },
                    );
                }
                "RCGameDetailsMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            rc_game_details_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "DownloadMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            download_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "QuitMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            quit_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                    window.mark_quit_menu_system_callback();
                }
                "SaveLoadMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            save_load_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "PopupReplaySystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_manager(|manager, effects| {
                                popup_replay_system(window, msg, data1, data2, manager, effects)
                            })
                        },
                    );
                }
                "ReplayMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            replay_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                    window.mark_replay_menu_system_callback();
                    if let Some(state) = self.replay_menu.as_ref() {
                        window.set_user_data(state.clone());
                    }
                }
                "ChallengeMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            challenge_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "DifficultySelectSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            difficulty_select_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "KeyboardOptionsMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            keyboard_options_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "GameSpyPlayerInfoOverlaySystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_player_info_system(window, msg, data1, data2)
                        },
                    );
                }
                "ScoreScreenSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            score_screen_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "SkirmishMapSelectMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_manager(|manager, effects| {
                                skirmish_map_select_menu_system(
                                    window, msg, data1, data2, manager, effects,
                                )
                            })
                        },
                    );
                }
                "SkirmishGameOptionsMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            skirmish_game_options_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "LanMapSelectMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_manager(|manager, effects| {
                                lan_map_select_menu_system(
                                    window, msg, data1, data2, manager, effects,
                                )
                            })
                        },
                    );
                }
                "LanGameOptionsMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            lan_game_options_menu_system(window, msg, data1, data2, effects)
                        },
                    );
                }
                "GameInfoWindowSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            game_info_window_system(window, msg, data1, data2)
                        },
                    );
                }
                "NetworkDirectConnectSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            network_direct_connect_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLoginMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_login_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLocaleSelectSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_locale_select_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLMessageWindowSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_message_window_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLBuddyOverlaySystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_buddy_overlay_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLBuddyOverlayRCMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_buddy_overlay_rc_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "PopupBuddyNotificationSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_buddy_notification_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLStatusMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_status_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLWelcomeMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_welcome_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLobbyMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_lobby_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLadderScreenSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_ladder_screen_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLMapSelectMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_map_select_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLGameSetupMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_game_setup_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLQuickMatchMenuSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_quick_match_menu_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLQMScoreScreenSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_qm_score_screen_system(window, msg, data1, data2)
                        },
                    );
                }
                "WOLCustomScoreScreenSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_custom_score_screen_system(window, msg, data1, data2)
                        },
                    );
                }
                "PassMessagesToParentSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            if msg == WindowMessage::Create
                                || msg == WindowMessage::Destroy
                                || msg == WindowMessage::ScriptCreate
                            {
                                return WindowMsgHandled::Ignored;
                            }

                            if let Some(parent) = window.get_parent() {
                                parent
                                    .borrow_mut()
                                    .send_system_message(msg, data1, data2, effects)
                            } else {
                                WindowMsgHandled::Ignored
                            }
                        },
                    );
                }
                "PassSelectedButtonsToParentSystem" => {
                    window.set_system_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            if msg != WindowMessage::GadgetSelected
                                && msg != WindowMessage::GadgetRightClick
                                && msg != WindowMessage::GadgetMouseEntering
                                && msg != WindowMessage::GadgetMouseLeaving
                                && msg != WindowMessage::GadgetEditDone
                            {
                                return WindowMsgHandled::Ignored;
                            }

                            if let Some(parent) = window.get_parent() {
                                parent
                                    .borrow_mut()
                                    .send_system_message(msg, data1, data2, effects)
                            } else {
                                WindowMsgHandled::Ignored
                            }
                        },
                    );
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
                    window.set_input_callback_with_message_sink(|window, msg, d1, d2, _effects| {
                        default_input_callback(window, msg, d1, d2)
                    });
                }
                "BeaconWindowInput" => {
                    window.set_input_callback(|window, msg, d1, d2| {
                        beacon_window_input(window, msg, d1, d2)
                    });
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
                    window.set_input_callback_with_message_sink(|window, msg, d1, d2, _effects| {
                        default_input_callback(window, msg, d1, d2)
                    });
                }
                "GameWinBlockInput" => {
                    window.set_input_callback(|_window, msg, _data1, _data2| match msg {
                        WindowMessage::Char | WindowMessage::MousePos => WindowMsgHandled::Ignored,
                        _ => WindowMsgHandled::Handled,
                    });
                }
                "ControlBarInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_control_bar_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let callbacks = system.get_callbacks();
                            with_arc_write(&callbacks, |callbacks| {
                                callbacks.system(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "LeftHUDInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_control_bar_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let callbacks = system.get_left_hud();
                            with_arc_write(&callbacks, |callbacks| {
                                callbacks.input(
                                    window,
                                    msg,
                                    data1,
                                    data2,
                                    effects.emitted_messages,
                                    effects.pending_control_bar_clicks.as_deref_mut(),
                                )
                            })
                        },
                    );
                }
                "DiplomacyInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            // ScriptCreate is routed synchronously to the root's
                            // input callback while each child is created
                            // (layout_load.rs:269-275) — i.e. while
                            // DiplomacySystem::toggle_diplomacy still holds the
                            // DiplomacyCallbacks write guard; re-acquiring it here
                            // self-deadlocks. C++ DiplomacyInput ignores everything
                            // except GWM_CHAR (Diplomacy.cpp:323-357), so creation
                            // traffic falls straight through to widget handling.
                            if msg == WindowMessage::ScriptCreate {
                                return WindowMsgHandled::Ignored;
                            }
                            let system = get_diplomacy_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let callbacks = system.get_callbacks();
                            with_arc_write(&callbacks, |callbacks| {
                                effects.with_manager(|manager, effects| {
                                    callbacks.input(window, msg, data1, data2, manager, effects)
                                })
                            })
                        },
                    );
                }
                "InGameChatInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_ingame_ui_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let chat = system.get_chat();
                            with_arc_write(&chat, |chat| {
                                effects.with_manager(|manager, effects| {
                                    chat.input(window, msg, data1, data2, manager, effects)
                                })
                            })
                        },
                    );
                }
                "ReplayControlInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let system = get_ingame_ui_system();
                            let system = system.read().unwrap_or_else(|e| e.into_inner());
                            let replay = system.get_replay();
                            with_arc_write(&replay, |replay| {
                                effects.with_manager(|manager, effects| {
                                    replay.input(window, msg, data1, data2, manager)
                                })
                            })
                        },
                    );
                }
                "MainMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            let raw_msg = map_window_message_to_main_menu(msg);
                            if raw_msg == 0 {
                                return WindowMsgHandled::Ignored;
                            }
                            if effects.with_main_menu_for_window(window, |menu, effects, window| {
                                menu.input_for_window(window, raw_msg, data1, data2, effects)
                            }) {
                                WindowMsgHandled::Handled
                            } else {
                                WindowMsgHandled::Ignored
                            }
                        },
                    );
                }
                "SinglePlayerMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_single_player_menu(|menu, effects| {
                                menu.input_with_effects(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "OptionsMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_options_menu(|menu, effects| {
                                menu.input(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "MapSelectMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_map_select_menu(|menu, effects| {
                                menu.input_with_effects(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "CreditsMenuInput" => {
                    window.set_input_callback_with_message_sink(|window, msg, d1, d2, effects| {
                        crate::gui::callbacks::owned_credits_menu::input(
                            window, msg, d1, d2, effects,
                        )
                    });
                }
                "LanLobbyMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_lan_lobby_menu(|menu, effects| {
                                menu.input(window, msg, data1, data2, effects)
                            })
                        },
                    );
                }
                "GeneralsExpPointsInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            generals_exp_points_input(window, msg, data1, data2)
                        },
                    );
                }
                "IMECandidateWindowInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            ime_candidate_window_input(window, msg, data1, data2)
                        },
                    );
                }
                "InGamePopupMessageInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            in_game_popup_message_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "PopupCommunicatorInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            effects.with_manager(|manager, effects| {
                                popup_communicator_input(
                                    window, msg, data1, data2, manager, effects,
                                )
                            })
                        },
                    );
                }
                "PopupJoinGameInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_join_game_input(window, msg, data1, data2)
                        },
                    );
                }
                "PopupHostGameInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_host_game_input(window, msg, data1, data2)
                        },
                    );
                }
                "PopupLadderSelectInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_ladder_select_input(window, msg, data1, data2)
                        },
                    );
                }
                "DownloadMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            download_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "SaveLoadMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            save_load_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "PopupReplayInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_replay_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "ReplayMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            replay_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "ChallengeMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            challenge_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "DifficultySelectInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            difficulty_select_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "KeyboardOptionsMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            keyboard_options_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "GameSpyPlayerInfoOverlayInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            popup_player_info_input(window, msg, data1, data2)
                        },
                    );
                }
                "ScoreScreenInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            score_screen_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "SkirmishMapSelectMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            skirmish_map_select_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "SkirmishGameOptionsMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            skirmish_game_options_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "LanMapSelectMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            lan_map_select_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "LanGameOptionsMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            lan_game_options_menu_input(window, msg, data1, data2, effects)
                        },
                    );
                }
                "NetworkDirectConnectInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            network_direct_connect_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLoginMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_login_menu_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLocaleSelectInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_locale_select_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLMessageWindowInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_message_window_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLBuddyOverlayInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_buddy_overlay_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLStatusMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_status_menu_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLWelcomeMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_welcome_menu_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLobbyMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_lobby_menu_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLLadderScreenInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_ladder_screen_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLMapSelectMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_map_select_menu_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLGameSetupMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_game_setup_menu_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLQuickMatchMenuInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_quick_match_menu_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLQMScoreScreenInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_qm_score_screen_input(window, msg, data1, data2)
                        },
                    );
                }
                "WOLCustomScoreScreenInput" => {
                    window.set_input_callback_with_message_sink(
                        |window, msg, data1, data2, effects| {
                            wol_custom_score_screen_input(window, msg, data1, data2)
                        },
                    );
                }
                other => {
                    warn!("Unimplemented input callback '{}', using default.", other);
                    window.set_input_callback_with_message_sink(|window, msg, d1, d2, _effects| {
                        default_input_callback(window, msg, d1, d2)
                    });
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
                    window.set_radar_draw_callback();
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
                    window.set_draw_callback(|window, inst, draw_context| {
                        ime_candidate_text_area_draw(window, inst, draw_context)
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
