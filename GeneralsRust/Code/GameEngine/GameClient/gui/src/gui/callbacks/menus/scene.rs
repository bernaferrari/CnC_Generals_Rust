use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::callbacks::menus::challenge_menu::ChallengeMenuPort;
use crate::gui::callbacks::menus::credits_menu::CreditsMenuPort;
use crate::gui::callbacks::menus::difficulty_select::{DifficultyChoicePort, DifficultySelectPort};
use crate::gui::callbacks::menus::disconnect_window::DisconnectWindowPort;
use crate::gui::callbacks::menus::download_menu::DownloadMenuPort;
use crate::gui::callbacks::menus::establish_connections_window::EstablishConnectionsWindowPort;
use crate::gui::callbacks::menus::game_info_window::GameInfoWindowPort;
use crate::gui::callbacks::menus::keyboard_options_menu::{
    KeyboardOptionsMenuPort, MappableKeyCategory, ModifierKind,
};
use crate::gui::callbacks::menus::lan_game_options_menu::LanGameOptionsMenuPort;
use crate::gui::callbacks::menus::lan_lobby_menu::LanLobbyMenuPort;
use crate::gui::callbacks::menus::lan_map_select_menu::LanMapSelectMenuPort;
use crate::gui::callbacks::menus::main_menu::{
    CampaignSidePort, MainMenuDropdownPort, MainMenuPort,
};
use crate::gui::callbacks::menus::map_select_menu::MapSelectMenuPort;
use crate::gui::callbacks::menus::network_direct_connect::NetworkDirectConnectPort;
use crate::gui::callbacks::menus::options_menu::{OptionsMenuPort, OptionsTabPort};
use crate::gui::callbacks::menus::popup_communicator::PopupCommunicatorPort;
use crate::gui::callbacks::menus::popup_host_game::PopupHostGamePort;
use crate::gui::callbacks::menus::popup_join_game::PopupJoinGamePort;
use crate::gui::callbacks::menus::popup_ladder_select::PopupLadderSelectPort;
use crate::gui::callbacks::menus::popup_player_info::PopupPlayerInfoPort;
use crate::gui::callbacks::menus::popup_replay::PopupReplayPort;
use crate::gui::callbacks::menus::popup_save_load::{PopupSaveLoadPort, SaveLoadModalPort};
use crate::gui::callbacks::menus::quit_menu::QuitMenuPort;
use crate::gui::callbacks::menus::replay_menu::{ReplayMenuPort, ReplayPromptPort};
use crate::gui::callbacks::menus::score_screen::ScoreScreenPort;
use crate::gui::callbacks::menus::single_player_menu::SinglePlayerMenuPort;
use crate::gui::callbacks::menus::skirmish_game_options_menu::SkirmishGameOptionsMenuPort;
use crate::gui::callbacks::menus::skirmish_map_select_menu::SkirmishMapSelectPort;
use crate::gui::callbacks::menus::wol_buddy_overlay::WolBuddyOverlayPort;
use crate::gui::callbacks::menus::wol_custom_score_screen::WolCustomScoreScreenPort;
use crate::gui::callbacks::menus::wol_game_setup_menu::WolGameSetupMenuPort;
use crate::gui::callbacks::menus::wol_ladder_screen::WolLadderScreenPort;
use crate::gui::callbacks::menus::wol_lobby_menu::WolLobbyMenuPort;
use crate::gui::callbacks::menus::wol_locale_select_popup::WolLocaleSelectPopupPort;
use crate::gui::callbacks::menus::wol_login_menu::WolLoginMenuPort;
use crate::gui::callbacks::menus::wol_map_select_menu::WolMapSelectMenuPort;
use crate::gui::callbacks::menus::wol_message_window::WolMessageWindowPort;
use crate::gui::callbacks::menus::wol_qm_score_screen::WolQmScoreScreenPort;
use crate::gui::callbacks::menus::wol_quick_match_menu::WolQuickMatchMenuPort;
use crate::gui::callbacks::menus::wol_status_menu::WolStatusMenuPort;
use crate::gui::callbacks::menus::wol_welcome_menu::WolWelcomeMenuPort;
use crate::gui::gadget::{
    gadget_check_box, gadget_combo_box, gadget_horizontal_slider, gadget_list_box,
    gadget_progress_bar, gadget_push_button, gadget_text_entry, gadget_vertical_slider,
};
use crate::gui::source_catalog::MenuScreenPort;

pub fn render_screen(screen: MenuScreenPort) -> AnyElement {
    match screen.key {
        "MainMenu" => render_main_menu_screen(screen),
        "SinglePlayerMenu" => render_single_player_screen(screen),
        "OptionsMenu" => render_options_screen(screen),
        "KeyboardOptionsMenu" => render_keyboard_options_screen(screen),
        "MapSelectMenu" | "SkirmishMapSelectMenu" | "LanMapSelectMenu" | "WOLMapSelectMenu" => {
            render_map_select_screen(screen)
        }
        "ReplayMenu" => render_replay_menu_screen(screen),
        "ChallengeMenu" => render_challenge_screen(screen),
        "SkirmishGameOptionsMenu" | "LanGameOptionsMenu" | "WOLGameSetupMenu" => {
            render_skirmish_setup_screen(screen)
        }
        "LanLobbyMenu" | "WOLLobbyMenu" => render_lobby_screen(screen),
        "WOLLoginMenu" => render_wol_login_screen(screen),
        "WOLWelcomeMenu" => render_wol_welcome_screen(screen),
        "WOLStatusMenu" => render_wol_status_screen(screen),
        "WOLMessageWindow" => render_wol_message_screen(screen),
        "WOLLadderScreen" => render_wol_ladder_screen(screen),
        "WOLQuickMatchMenu" => render_wol_quick_match_screen(screen),
        "WOLBuddyOverlay" => render_wol_buddy_overlay_screen(screen),
        "WOLCustomScoreScreen" => render_wol_custom_score_screen(screen),
        "WOLQMScoreScreen" => render_wol_qm_score_screen(screen),
        "SaveLoadMenu" => render_save_load_screen(screen),
        "PopupHostGame" => render_popup_host_game_screen(screen),
        "PopupJoinGame" => render_popup_join_game_screen(screen),
        "PopupCommunicator" => render_popup_communicator_screen(screen),
        "PopupLadderSelect" => render_popup_ladder_select_screen(screen),
        "PopupPlayerInfo" => render_popup_player_info_screen(screen),
        "PopupReplay" => render_popup_replay_screen(screen),
        "DifficultySelect" => render_difficulty_select_screen(screen),
        "DisconnectWindow" => render_disconnect_window_screen(screen),
        "EstablishConnectionsWindow" => render_establish_connections_screen(screen),
        "GameInfoWindow" => render_game_info_screen(screen),
        "WOLLocaleSelect" => render_wol_locale_select_screen(screen),
        "ScoreScreen" => render_score_screen(screen),
        "CreditsMenu" => render_credits_screen(screen),
        "DownloadMenu" => render_download_screen(screen),
        "QuitMenu" => render_quit_menu_screen(screen),
        "NetworkDirectConnect" => render_network_direct_connect_screen(screen),
        _ => screen_frame(
            screen.title,
            screen.summary,
            vec![
                static_text("C++ Source", screen.record.cpp_relative_path),
                static_text("Rust Module", screen.record.rust_module_path),
                static_text("Summary", screen.summary),
            ],
        ),
    }
}

fn render_main_menu_screen(screen: MenuScreenPort) -> AnyElement {
    let state = MainMenuPort::sample();
    let selected_side = state.selected_campaign.map(CampaignSidePort::label);

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children([
                    menu_button("Single Player", true),
                    menu_button("Skirmish", false),
                    menu_button("Multiplayer", false),
                    menu_button("Load Replay", false),
                    menu_button("Options", state.options_menu_visible),
                    menu_button("Exit", state.quit_requested),
                ])
                .into_any_element(),
            div()
                .flex()
                .gap_2()
                .flex_wrap()
                .children([
                    status_pill("Dropdown", state.drop_down.label()),
                    status_pill(
                        "Campaign",
                        if state.campaign_selected {
                            selected_side.unwrap_or("Selected")
                        } else {
                            "None"
                        },
                    ),
                    status_pill(
                        "Transitions",
                        if state.dont_allow_transitions {
                            "Locked"
                        } else {
                            "Ready"
                        },
                    ),
                    status_pill(
                        "Shell Map",
                        if state.shell_map_visible {
                            "Shown"
                        } else {
                            "Hidden"
                        },
                    ),
                ])
                .into_any_element(),
            render_main_menu_flow(&state),
            render_main_menu_recent_saves(&state),
        ],
    )
}

fn render_keyboard_options_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = KeyboardOptionsMenuPort::sample();
    state.select_category(MappableKeyCategory::Control);
    state.do_key_down(ModifierKind::Ctrl);
    state.assign_key('F');

    let selected = state.selected_command();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .gap_2()
                .children([
                    menu_button(
                        "Control",
                        state.selected_category == MappableKeyCategory::Control,
                    ),
                    menu_button(
                        "Selection",
                        state.selected_category == MappableKeyCategory::Selection,
                    ),
                    menu_button(
                        "Interface",
                        state.selected_category == MappableKeyCategory::Interface,
                    ),
                    menu_button("Team", state.selected_category == MappableKeyCategory::Team),
                ])
                .into_any_element(),
            command_list(
                "Commands",
                state
                    .commands
                    .iter()
                    .filter(|command| command.category == state.selected_category)
                    .map(|command| {
                        format!(
                            "{} [{}]",
                            command.display_name,
                            command.current_hotkey_display()
                        )
                    })
                    .collect(),
            ),
            static_text(
                "Selected Command",
                selected
                    .map(|command| command.display_name.clone())
                    .unwrap_or_else(|| "None".to_string()),
            ),
            static_text(
                "Description",
                selected
                    .map(|command| command.description.clone())
                    .unwrap_or_else(|| "No description".to_string()),
            ),
            static_text("Assign Hotkey", state.assign_text),
        ],
    )
}

fn render_single_player_screen(screen: MenuScreenPort) -> AnyElement {
    let state = SinglePlayerMenuPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .gap_2()
                .children([
                    menu_button(
                        "New Campaign",
                        state.pending_shell_push.as_deref() == Some("Menus/MapSelectMenu.wnd"),
                    ),
                    menu_button("Load", false),
                    menu_button("Back", state.pop_requested),
                ])
                .into_any_element(),
            div()
                .flex()
                .gap_2()
                .flex_wrap()
                .children([
                    status_pill("Focus", state.focused_control.label()),
                    status_pill(
                        "Shell Map",
                        if state.shell_map_visible {
                            "Shown"
                        } else {
                            "Hidden"
                        },
                    ),
                    status_pill(
                        "Shutdown",
                        if state.is_shutting_down {
                            "Pending"
                        } else {
                            "Active"
                        },
                    ),
                ])
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(section_title("Animate Manager"))
                .children(state.animations.iter().map(|animation| {
                    status_row(
                        animation.control.label(),
                        format!(
                            "{} @ {}ms",
                            animation.animation.label(),
                            animation.start_delay_ms
                        ),
                    )
                }))
                .into_any_element(),
            static_text(
                "Escape Routing",
                "ESC simulates GBM_SELECTED on ButtonBack, matching the original shell callback.",
            ),
        ],
    )
}

fn render_challenge_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = ChallengeMenuPort::sample();
    state.select_general(0);
    state.update_bio(4, 2);

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            command_list(
                "Generals",
                state
                    .generals
                    .iter()
                    .enumerate()
                    .map(|(index, general)| {
                        format!(
                            "{}{}{}",
                            if Some(index) == state.selected_general {
                                "* "
                            } else {
                                ""
                            },
                            general.name,
                            if general.enabled { "" } else { " (locked)" }
                        )
                    })
                    .collect(),
            ),
            static_text(
                "Campaign",
                state
                    .selected_general
                    .and_then(|i| state.generals.get(i))
                    .map(|g| g.campaign.clone())
                    .unwrap_or_default(),
            ),
            static_text("Bio Readout", state.current_readout()),
            static_text("Play Enabled", if state.can_play { "Yes" } else { "No" }),
        ],
    )
}

fn render_options_screen(screen: MenuScreenPort) -> AnyElement {
    let state = OptionsMenuPort::sample();
    let tabs = [
        OptionsTabPort::Audio,
        OptionsTabPort::Video,
        OptionsTabPort::Gameplay,
        OptionsTabPort::Controls,
        OptionsTabPort::AdvancedDisplay,
    ];

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .gap_2()
                .children(
                    tabs.into_iter()
                        .map(|tab| menu_button(tab.label(), tab == state.active_tab)),
                )
                .into_any_element(),
            div()
                .flex()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(gadget_horizontal_slider::render(
                            &format!("Music Volume: {}%", pct(state.music_volume)),
                            &horizontal_slider_state(state.music_volume),
                        ))
                        .child(gadget_horizontal_slider::render(
                            &format!("FX Volume: {}%", pct(state.sfx_volume)),
                            &horizontal_slider_state(state.sfx_volume),
                        ))
                        .child(gadget_horizontal_slider::render(
                            &format!("Voice Volume: {}%", pct(state.voice_volume)),
                            &horizontal_slider_state(state.voice_volume),
                        ))
                        .child(gadget_combo_box::render(&combo_box_state(
                            format!("{}x{}", state.resolution.0, state.resolution.1),
                            &["1024x768", "1280x720", "1600x900", "1920x1080"],
                        )))
                        .child(gadget_combo_box::render(&combo_box_state(
                            format!("{}x AA", state.anti_aliasing),
                            &["0x AA", "2x AA", "4x AA", "8x AA"],
                        )))
                        .child(gadget_horizontal_slider::render(
                            &format!("Scroll Speed: {}%", pct(state.scroll_speed)),
                            &horizontal_slider_state(state.scroll_speed),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(gadget_check_box::render(
                            "Retaliation mode",
                            &check_box_state(state.retaliation_mode),
                        ))
                        .child(gadget_check_box::render(
                            "Double-click attack move",
                            &check_box_state(state.double_click_attack_move),
                        ))
                        .child(gadget_check_box::render(
                            "Alternate mouse",
                            &check_box_state(state.alternate_mouse),
                        ))
                        .child(gadget_check_box::render(
                            "Language filter",
                            &check_box_state(state.language_filter),
                        ))
                        .child(gadget_check_box::render(
                            "Use camera in replays",
                            &check_box_state(state.use_camera_in_replays),
                        ))
                        .child(gadget_check_box::render(
                            "Save camera in replays",
                            &check_box_state(state.save_camera_in_replays),
                        ))
                        .child(gadget_check_box::render(
                            "Cloud shadows",
                            &check_box_state(state.cloud_shadows),
                        ))
                        .child(gadget_check_box::render(
                            "Ground lighting",
                            &check_box_state(state.ground_lighting),
                        ))
                        .child(gadget_check_box::render(
                            "Heat effects",
                            &check_box_state(state.heat_effects),
                        ))
                        .child(gadget_check_box::render(
                            "Unlock FPS",
                            &check_box_state(state.unlock_fps),
                        ))
                        .child(gadget_vertical_slider::render(&vertical_slider_state(
                            state.gamma,
                        ))),
                )
                .into_any_element(),
            div()
                .flex()
                .gap_2()
                .flex_wrap()
                .children([
                    status_pill("LAN IP", &state.lan_ip),
                    status_pill("Online IP", &state.online_ip),
                    status_pill("Particle Cap", &state.particle_cap.to_string()),
                    status_pill("Campaign Difficulty", state.campaign_difficulty.label()),
                    status_pill("Texture Reduction", &state.texture_reduction.to_string()),
                ])
                .into_any_element(),
        ],
    )
}

fn render_skirmish_setup_screen(screen: MenuScreenPort) -> AnyElement {
    let (state, setup_type, extra_lines) = match screen.key {
        "LanGameOptionsMenu" => {
            let state = LanGameOptionsMenuPort::sample();
            (
                state.setup,
                "LAN Setup".to_string(),
                vec![format!("Local Address: {}", state.local_address)],
            )
        }
        "WOLGameSetupMenu" => {
            let state = WolGameSetupMenuPort::sample();
            (
                state.setup,
                "WOL Setup".to_string(),
                vec![
                    format!(
                        "Ladder Game: {}",
                        if state.ladder_game { "Yes" } else { "No" }
                    ),
                    format!(
                        "Stats Reporting: {}",
                        if state.stats_reporting {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    ),
                ],
            )
        }
        _ => (
            SkirmishGameOptionsMenuPort::sample(),
            "Skirmish Setup".to_string(),
            Vec::new(),
        ),
    };

    let mut body = vec![
        static_text("Setup Type", setup_type),
        static_text("Map", state.map_name),
        static_text("Player Name", state.player_name),
        static_text("Game Speed", format!("{}%", state.game_speed)),
        static_text("Starting Cash", format!("${}", state.starting_cash)),
        static_text(
            "Superweapons",
            if state.superweapons_restricted {
                "Restricted"
            } else {
                "Enabled"
            },
        ),
        command_list(
            "Slots",
            state
                .slots
                .iter()
                .enumerate()
                .map(|(index, slot)| {
                    format!(
                        "{}{} / {} / {} / team {} / start {}",
                        if index == state.selected_slot {
                            "* "
                        } else {
                            ""
                        },
                        slot.player_name,
                        slot.faction,
                        slot.color,
                        slot.team,
                        slot.start_pos
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "Random".to_string())
                    )
                })
                .collect(),
        ),
    ];
    body.extend(
        extra_lines
            .into_iter()
            .map(|line| static_text("Session", line)),
    );

    screen_frame(screen.title, screen.summary, body)
}

fn render_replay_menu_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = ReplayMenuPort::sample();
    let _ = state.select_index(2);
    let _ = state.load_selected();

    let prompt = state.pending_prompt.clone();
    let selected_summary = state
        .selected_index
        .and_then(|index| state.entries.get(index))
        .map(|entry| format!("{} ({})", entry.replay_name, entry.version))
        .unwrap_or_else(|| "Nothing selected".to_string());

    let mut body = vec![
        div()
            .flex()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("Replay Files"))
                    .child(gadget_list_box::render(&replay_list_box_state(&state))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("Selected Replay"))
                    .child(status_row("Selected", selected_summary))
                    .child(status_row(
                        "Input Focus",
                        if state.take_input_focus(true) {
                            "Accepted".to_string()
                        } else {
                            "Rejected".to_string()
                        },
                    ))
                    .child(status_row(
                        "Entry Transition",
                        state
                            .active_transition_group
                            .as_deref()
                            .unwrap_or("ReplayMenuFade pending")
                            .to_string(),
                    ))
                    .child(status_row(
                        "Gadget Parent",
                        if state.gadget_parent_hidden {
                            "Hidden on init".to_string()
                        } else {
                            "Visible".to_string()
                        },
                    )),
            )
            .into_any_element(),
        div()
            .flex()
            .gap_2()
            .children([
                menu_button("Load", false),
                menu_button(
                    "Delete",
                    matches!(prompt, Some(ReplayPromptPort::DeleteConfirm { .. })),
                ),
                menu_button(
                    "Copy",
                    matches!(prompt, Some(ReplayPromptPort::CopyConfirm { .. })),
                ),
                menu_button("Back", state.back_requested),
            ])
            .into_any_element(),
    ];

    if let Some(prompt) = prompt {
        body.push(render_replay_prompt(prompt));
    }

    screen_frame(screen.title, screen.summary, body)
}

fn render_map_select_screen(screen: MenuScreenPort) -> AnyElement {
    let (mut state, selection_type, extra_lines) = match screen.key {
        "SkirmishMapSelectMenu" => {
            let state = SkirmishMapSelectPort::sample();
            (
                state.map_select,
                "Skirmish".to_string(),
                vec![format!(
                    "Official Maps Only: {}",
                    if state.official_maps_only {
                        "Yes"
                    } else {
                        "No"
                    }
                )],
            )
        }
        "LanMapSelectMenu" => {
            let state = LanMapSelectMenuPort::sample();
            (
                state.map_select,
                "LAN".to_string(),
                vec![state.direct_connect_hint],
            )
        }
        "WOLMapSelectMenu" => {
            let state = WolMapSelectMenuPort::sample();
            (
                state.map_select,
                "WOL".to_string(),
                vec![format!("Rotation: {}", state.rotation_name)],
            )
        }
        _ => (
            MapSelectMenuPort::sample(),
            "Campaign".to_string(),
            Vec::new(),
        ),
    };
    let _ = state.select_map(1);
    state.set_difficulty(crate::gui::callbacks::menus::main_menu::GameDifficultyPort::Hard);

    let mut body = vec![
        static_text("Selection Type", selection_type),
        div()
            .flex()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("Available Maps"))
                    .child(gadget_list_box::render(&map_select_list_box_state(&state)))
                    .into_any_element(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("Selection State"))
                    .child(status_row(
                        "Map Directory",
                        if state.uses_system_map_dir {
                            "System Maps".to_string()
                        } else {
                            "User Maps".to_string()
                        },
                    ))
                    .child(status_row(
                        "AI Difficulty",
                        state.ai_difficulty.label().to_string(),
                    ))
                    .child(status_row(
                        "Pending File",
                        state
                            .pending_file
                            .clone()
                            .unwrap_or_else(|| "Nothing staged".to_string()),
                    ))
                    .child(gadget_check_box::render(
                        "Solo maps",
                        &check_box_state(state.show_solo_maps),
                    ))
                    .child(gadget_check_box::render(
                        "Back requested",
                        &check_box_state(state.pop_requested),
                    )),
            )
            .into_any_element(),
    ];
    body.extend(
        extra_lines
            .into_iter()
            .map(|line| static_text("Notes", line)),
    );
    body.push(
        div()
            .flex()
            .gap_2()
            .children([
                menu_button("Back", false),
                menu_button("OK", false),
                menu_button("Easy", false),
                menu_button("Medium", false),
                menu_button("Hard", true),
            ])
            .into_any_element(),
    );

    screen_frame(screen.title, screen.summary, body)
}

fn render_lobby_screen(screen: MenuScreenPort) -> AnyElement {
    let (mut state, lobby_type, extra_lines) = match screen.key {
        "WOLLobbyMenu" => {
            let state = WolLobbyMenuPort::sample();
            (
                state.lobby,
                "WOL Lobby".to_string(),
                vec![
                    state.room_id,
                    format!(
                        "Ranked Room: {}",
                        if state.ranked_room { "Yes" } else { "No" }
                    ),
                ],
            )
        }
        _ => (
            LanLobbyMenuPort::sample(),
            "LAN Lobby".to_string(),
            Vec::new(),
        ),
    };
    state.chat_entry = "ready when you are".to_string();
    let _ = state.send_chat();
    state.update();
    state.update();

    let mut body = vec![
        static_text("Lobby Type", lobby_type),
        div()
            .flex()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("Players"))
                    .child(gadget_list_box::render(&lobby_players_list_box_state(
                        &state,
                    )))
                    .into_any_element(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("Lobby Chat"))
                    .children(state.chat_history.iter().map(|line| {
                        div()
                            .text_sm()
                            .text_color(rgb(0x8ea2b4))
                            .child(line.clone())
                    }))
                    .child(gadget_text_entry::render(&text_entry_state(
                        "ready when you are",
                    )))
                    .child(status_row(
                        "Transition",
                        state
                            .active_transition_group
                            .clone()
                            .unwrap_or_else(|| "LanLobbyMenuFade pending".to_string()),
                    )),
            )
            .into_any_element(),
    ];
    body.extend(
        extra_lines
            .into_iter()
            .map(|line| static_text("Lobby", line)),
    );
    body.push(
        div()
            .flex()
            .gap_2()
            .children([
                menu_button("Host", false),
                menu_button("Join", false),
                menu_button("Direct Connect", false),
                menu_button("Back", false),
            ])
            .into_any_element(),
    );

    screen_frame(screen.title, screen.summary, body)
}

fn render_wol_login_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = WolLoginMenuPort::sample();
    let _ = state.attempt_login(100);
    let _ = state.check_timeout(10_200);

    screen_frame(
        screen.title,
        screen.summary,
        vec![div()
            .flex()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("Stored Accounts"))
                    .child(gadget_list_box::render(&stored_accounts_list_box_state(
                        &state,
                    )))
                    .into_any_element(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(status_row("Email", state.email.clone()))
                    .child(status_row("Nick", state.nick.clone()))
                    .child(gadget_check_box::render(
                        "Remember account",
                        &check_box_state(state.remember_account),
                    ))
                    .child(gadget_check_box::render(
                        "Password stored",
                        &check_box_state(state.password_present),
                    ))
                    .child(status_row("Status", state.status_message.clone())),
            )
            .into_any_element()],
    )
}

fn render_wol_welcome_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolWelcomeMenuPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Server", state.server_name),
            static_text("Players Online", state.players_online.to_string()),
            static_text(
                "Ladder",
                format!(
                    "{}W {}L / {} pts / rank {} / disconnects {}",
                    state.ladder_wins,
                    state.ladder_losses,
                    state.ladder_points,
                    state.ladder_rank,
                    state.disconnects
                ),
            ),
            command_list("Info", state.info_items),
        ],
    )
}

fn render_wol_status_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolStatusMenuPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Service", state.service_name),
            command_list("Status", state.status_lines),
            static_text(
                "Can Disconnect",
                if state.can_disconnect { "Yes" } else { "No" },
            ),
        ],
    )
}

fn render_wol_message_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolMessageWindowPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![command_list(
            "Inbox",
            state
                .messages
                .iter()
                .enumerate()
                .map(|(index, message)| {
                    format!(
                        "{}{} - {}",
                        if state.selected_message == Some(index) {
                            "* "
                        } else {
                            ""
                        },
                        message.from,
                        message.subject
                    )
                })
                .collect(),
        )],
    )
}

fn render_save_load_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = PopupSaveLoadPort::sample();
    let _ = state.request_load();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            command_list(
                "Saves",
                state
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        format!(
                            "{}{} - {}",
                            if state.selected_index == Some(index) { "* " } else { "" },
                            entry.filename,
                            entry.description
                        )
                    })
                    .collect(),
            ),
            static_text(
                "Mode",
                match state.layout_type {
                    crate::gui::callbacks::menus::popup_save_load::SaveLoadLayoutTypePort::SaveAndLoad => {
                        "Save and Load"
                    }
                    crate::gui::callbacks::menus::popup_save_load::SaveLoadLayoutTypePort::LoadOnly => {
                        "Load Only"
                    }
                },
            ),
            static_text(
                "Save Enabled",
                if state.can_save() { "Yes" } else { "No" },
            ),
            static_text(
                "Load Enabled",
                if state.can_load() { "Yes" } else { "No" },
            ),
            static_text(
                "Active Modal",
                match state.active_modal {
                    SaveLoadModalPort::None => "None",
                    SaveLoadModalPort::OverwriteConfirm => "Overwrite Confirm",
                    SaveLoadModalPort::LoadConfirm => "Load Confirm",
                    SaveLoadModalPort::SaveDescription => "Save Description",
                    SaveLoadModalPort::DeleteConfirm => "Delete Confirm",
                },
            ),
        ],
    )
}

fn render_popup_host_game_screen(screen: MenuScreenPort) -> AnyElement {
    let state = PopupHostGamePort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Game Name", state.game_name),
            static_text("Description", state.game_description),
            static_text("Ladder", state.selected_ladder),
            static_text("Game Password", state.game_password),
            static_text(
                "Allow Observers",
                if state.allow_observers { "Yes" } else { "No" },
            ),
            static_text("Use Stats", if state.use_stats { "Yes" } else { "No" }),
            static_text(
                "Limit Armies",
                if state.limit_armies { "Yes" } else { "No" },
            ),
        ],
    )
}

fn render_popup_join_game_screen(screen: MenuScreenPort) -> AnyElement {
    let state = PopupJoinGamePort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Game Name", state.game_name),
            static_text("Password", state.password),
            static_text("Can Join", if state.can_join { "Yes" } else { "No" }),
        ],
    )
}

fn render_popup_communicator_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = PopupCommunicatorPort::sample();
    let _ = state.send();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Recipient", state.recipient),
            command_list("Conversation", state.history),
            static_text(
                "Draft",
                if state.message_entry.is_empty() {
                    "Sent"
                } else {
                    "Pending"
                },
            ),
        ],
    )
}

fn render_popup_ladder_select_screen(screen: MenuScreenPort) -> AnyElement {
    let state = PopupLadderSelectPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            command_list(
                "Ladders",
                state
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        format!(
                            "{}{} - {}",
                            if index == state.selected_index {
                                "* "
                            } else {
                                ""
                            },
                            entry.name,
                            entry.description
                        )
                    })
                    .collect(),
            ),
            static_text(
                "Selection",
                state
                    .current()
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| "None".to_string()),
            ),
        ],
    )
}

fn render_popup_player_info_screen(screen: MenuScreenPort) -> AnyElement {
    let state = PopupPlayerInfoPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Player", state.player_name),
            static_text(
                "Faction / Clan",
                format!("{} / {}", state.faction, state.clan),
            ),
            static_text(
                "Record",
                format!(
                    "{} wins / {} losses / {} disconnects",
                    state.wins, state.losses, state.disconnects
                ),
            ),
            static_text("Status", state.online_status),
        ],
    )
}

fn render_popup_replay_screen(screen: MenuScreenPort) -> AnyElement {
    let state = PopupReplayPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Replay Name", state.replay_name),
            static_text("Description", state.description),
            static_text(
                "Overwrite Existing",
                if state.overwrite_existing {
                    "Yes"
                } else {
                    "No"
                },
            ),
            static_text("Can Save", if state.can_save { "Yes" } else { "No" }),
        ],
    )
}

fn render_difficulty_select_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = DifficultySelectPort::sample();
    state.confirm();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .gap_2()
                .children([
                    menu_button("Easy", state.selected == DifficultyChoicePort::Easy),
                    menu_button("Medium", state.selected == DifficultyChoicePort::Medium),
                    menu_button("Hard", state.selected == DifficultyChoicePort::Hard),
                ])
                .into_any_element(),
            static_text("Selected", state.selected.label()),
            static_text("Last Confirmed", state.last_confirmed.label()),
            command_list("Effects", state.description),
        ],
    )
}

fn render_disconnect_window_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = DisconnectWindowPort::sample();
    state.tick(500);
    let timed_out = state.timed_out();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Headline", state.headline),
            static_text("Reason", state.reason),
            static_text(
                "Reconnect Allowed",
                if state.reconnect_allowed { "Yes" } else { "No" },
            ),
            static_text(
                "Timeout",
                format!(
                    "{} / {} ms{}",
                    state.elapsed_ms,
                    state.timeout_ms,
                    if timed_out { " expired" } else { "" }
                ),
            ),
        ],
    )
}

fn render_establish_connections_screen(screen: MenuScreenPort) -> AnyElement {
    let state = EstablishConnectionsWindowPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text(
                "Peers",
                format!(
                    "{}/{} connected",
                    state.peers_connected, state.expected_peers
                ),
            ),
            command_list(
                "Steps",
                state
                    .steps
                    .iter()
                    .map(|step| {
                        format!(
                            "{}{}",
                            if step.completed { "[x] " } else { "[ ] " },
                            step.label
                        )
                    })
                    .collect(),
            ),
            static_text(
                "Cancel Requested",
                if state.cancel_requested { "Yes" } else { "No" },
            ),
        ],
    )
}

fn render_game_info_screen(screen: MenuScreenPort) -> AnyElement {
    let state = GameInfoWindowPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Game", state.game_name),
            static_text("Map", state.map_name),
            static_text("Host", state.host_name),
            static_text(
                "Players",
                format!("{}/{}", state.player_counts.0, state.player_counts.1),
            ),
            command_list("Rules", state.rule_lines),
            static_text(
                "Download Required",
                if state.download_required { "Yes" } else { "No" },
            ),
        ],
    )
}

fn render_wol_locale_select_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolLocaleSelectPopupPort::sample();
    let selected = state.selected_locale().unwrap_or("None").to_string();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            command_list("Locales", state.locales),
            static_text("Selected", selected),
            static_text("Route Region", state.route_region),
        ],
    )
}

fn render_score_screen(screen: MenuScreenPort) -> AnyElement {
    let state = ScoreScreenPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .gap_2()
                .children(
                    state
                        .metrics
                        .iter()
                        .map(|metric| stat_card(&metric.label, &metric.value)),
                )
                .into_any_element(),
            gadget_progress_bar::render("Overall rating", &progress_bar_state(state.rating)),
            static_text("Player", state.player_name),
            static_text("Match Result", state.result),
        ],
    )
}

fn render_credits_screen(screen: MenuScreenPort) -> AnyElement {
    let state = CreditsMenuPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Scroll Offset", state.scroll_offset.to_string()),
            command_list(
                "Credits",
                state
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        format!(
                            "{}{}",
                            if index == state.highlighted_line {
                                "* "
                            } else {
                                ""
                            },
                            line
                        )
                    })
                    .collect(),
            ),
        ],
    )
}

fn render_download_screen(screen: MenuScreenPort) -> AnyElement {
    let state = DownloadMenuPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Patch Server", state.patch_server),
            command_list(
                "Queue",
                state
                    .queue
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        format!(
                            "{}{} - {} ({}%)",
                            if index == state.selected_download {
                                "* "
                            } else {
                                ""
                            },
                            entry.label,
                            entry.status,
                            entry.progress_pct
                        )
                    })
                    .collect(),
            ),
            static_text("Overall Progress", format!("{}%", state.total_progress_pct)),
            static_text("Can Cancel", if state.can_cancel { "Yes" } else { "No" }),
            command_list("Notes", state.notes),
        ],
    )
}

fn render_quit_menu_screen(screen: MenuScreenPort) -> AnyElement {
    let state = QuitMenuPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Prompt", state.confirmation_text),
            static_text("In Match", if state.in_match { "Yes" } else { "No" }),
            static_text(
                "Unsaved Progress",
                if state.has_unsaved_progress {
                    "Yes"
                } else {
                    "No"
                },
            ),
            static_text("Default Focus", state.default_focus.label()),
        ],
    )
}

fn render_network_direct_connect_screen(screen: MenuScreenPort) -> AnyElement {
    let state = NetworkDirectConnectPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Host IP", state.host_ip),
            static_text("Port", state.port.to_string()),
            static_text("Nickname", state.nickname),
            static_text("Status", state.status_message),
        ],
    )
}

fn render_wol_ladder_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolLadderScreenPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Season", state.season_label),
            static_text("Local Rank", state.local_rank.to_string()),
            command_list(
                "Standings",
                state
                    .standings
                    .iter()
                    .map(|entry| {
                        format!("#{} {} ({})", entry.rank, entry.player_name, entry.points)
                    })
                    .collect(),
            ),
        ],
    )
}

fn render_wol_quick_match_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolQuickMatchMenuPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text("Preferred Faction", state.preferred_faction),
            command_list("Map Pool", state.map_pool),
            static_text("Queue State", state.queue_state),
            static_text(
                "Estimated Wait",
                format!("{} sec", state.estimated_wait_seconds),
            ),
        ],
    )
}

fn render_wol_buddy_overlay_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolBuddyOverlayPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![command_list(
            "Buddies",
            state
                .buddies
                .iter()
                .enumerate()
                .map(|(index, buddy)| {
                    format!(
                        "{}{} - {}{}",
                        if index == state.selected_index {
                            "* "
                        } else {
                            ""
                        },
                        buddy.name,
                        buddy.status,
                        if buddy.unread_messages == 0 {
                            String::new()
                        } else {
                            format!(" / {} unread", buddy.unread_messages)
                        }
                    )
                })
                .collect(),
        )],
    )
}

fn render_wol_custom_score_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolCustomScoreScreenPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text(
                "Result",
                format!(
                    "{} vs {}: {}",
                    state.player_name, state.opponent_name, state.result
                ),
            ),
            command_list(
                "Score Lines",
                state
                    .score_lines
                    .iter()
                    .map(|line| format!("{}: {}", line.label, line.value))
                    .collect(),
            ),
        ],
    )
}

fn render_wol_qm_score_screen(screen: MenuScreenPort) -> AnyElement {
    let state = WolQmScoreScreenPort::sample();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            static_text(
                "Rating",
                format!(
                    "{} -> {} ({:+})",
                    state.rating_before,
                    state.rating_after,
                    state.rating_delta()
                ),
            ),
            static_text("Streak", state.streak.to_string()),
            command_list("Summary", state.summary_lines),
        ],
    )
}

fn stat_card(label: &str, value: &str) -> AnyElement {
    div()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x101720))
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x8ea2b4))
                        .child(label.to_string()),
                )
                .child(value.to_string()),
        )
        .into_any_element()
}

fn screen_frame(title: &str, summary: &str, body: Vec<AnyElement>) -> AnyElement {
    div()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x0e1620))
        .flex()
        .flex_col()
        .gap_3()
        .child(title.to_string())
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(summary.to_string()),
        )
        .children(body)
        .into_any_element()
}

fn render_main_menu_flow(state: &MainMenuPort) -> AnyElement {
    let single_player_buttons = [
        menu_button(
            "Campaign",
            matches!(
                state.drop_down,
                MainMenuDropdownPort::Single | MainMenuDropdownPort::Difficulty
            ) && state.selected_campaign.is_some(),
        ),
        menu_button("Challenge", state.launch_challenge_menu),
        menu_button(
            "Load Game",
            state.last_shell_push.as_deref() == Some("Menus/SaveLoad.wnd"),
        ),
    ];

    let side_buttons = [
        menu_button(
            "USA",
            state.selected_campaign == Some(CampaignSidePort::Usa),
        ),
        menu_button(
            "GLA",
            state.selected_campaign == Some(CampaignSidePort::Gla),
        ),
        menu_button(
            "China",
            state.selected_campaign == Some(CampaignSidePort::China),
        ),
        menu_button(
            "Challenge",
            state.selected_campaign == Some(CampaignSidePort::Training),
        ),
    ];

    let difficulty_buttons = [
        menu_button("Easy", false),
        menu_button("Medium", false),
        menu_button("Hard", false),
    ];

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(section_title("Single Player Flow"))
        .child(
            div()
                .flex()
                .gap_2()
                .children(single_player_buttons)
                .into_any_element(),
        )
        .child(section_title("Campaign Side Selection"))
        .child(
            div()
                .flex()
                .gap_2()
                .flex_wrap()
                .children(side_buttons)
                .into_any_element(),
        )
        .child(section_title("Difficulty Selection"))
        .child(
            div()
                .flex()
                .gap_2()
                .children(difficulty_buttons)
                .into_any_element(),
        )
        .child(match &state.pending_game_start {
            Some(start) => static_text(
                "Pending Start",
                format!(
                    "{} on {}{}",
                    start.difficulty.label(),
                    start.map_name,
                    if start.opens_challenge_menu {
                        " -> ChallengeMenu"
                    } else {
                        ""
                    }
                ),
            ),
            None => static_text(
                "Pending Start",
                "Awaiting difficulty selection before shell reverse / game launch.",
            ),
        })
        .into_any_element()
}

fn render_main_menu_recent_saves(state: &MainMenuPort) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section_title("Selective Save / Load Buttons"))
        .children([
            gadget_check_box::render(
                "USA recent save",
                &check_box_state(state.selective_buttons.usa_recent_save),
            ),
            gadget_check_box::render(
                "USA load game",
                &check_box_state(state.selective_buttons.usa_load_game),
            ),
            gadget_check_box::render(
                "GLA recent save",
                &check_box_state(state.selective_buttons.gla_recent_save),
            ),
            gadget_check_box::render(
                "GLA load game",
                &check_box_state(state.selective_buttons.gla_load_game),
            ),
            gadget_check_box::render(
                "China recent save",
                &check_box_state(state.selective_buttons.china_recent_save),
            ),
            gadget_check_box::render(
                "China load game",
                &check_box_state(state.selective_buttons.china_load_game),
            ),
        ])
        .into_any_element()
}

fn render_replay_prompt(prompt: ReplayPromptPort) -> AnyElement {
    let (title, body, accent) = match prompt {
        ReplayPromptPort::NoSelection { title, body } => (title, body, "Selection required"),
        ReplayPromptPort::OlderVersion { title, body, .. } => (title, body, "Version mismatch"),
        ReplayPromptPort::DeleteConfirm { title, body, .. } => (title, body, "Delete confirm"),
        ReplayPromptPort::CopyConfirm { title, body, .. } => (title, body, "Copy confirm"),
    };

    div()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x5d4321))
        .bg(rgb(0x19140e))
        .flex()
        .flex_col()
        .gap_2()
        .child(title)
        .child(div().text_sm().text_color(rgb(0xd6b179)).child(accent))
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(body))
        .into_any_element()
}

fn menu_button(label: &str, active: bool) -> AnyElement {
    gadget_push_button::render(
        label,
        &gadget_push_button::PushButtonState {
            selected: active,
            hilited: active,
            ..Default::default()
        },
    )
}

fn section_title(title: &str) -> AnyElement {
    div()
        .text_sm()
        .text_color(rgb(0xd6b179))
        .child(title.to_string())
        .into_any_element()
}

fn status_pill(label: &str, value: impl ToString) -> AnyElement {
    div()
        .px_3()
        .py_1()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x101720))
        .flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(label.to_string()),
        )
        .child(value.to_string())
        .into_any_element()
}

fn status_row(label: &str, value: impl ToString) -> AnyElement {
    div()
        .flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(label.to_string()),
        )
        .child(value.to_string())
        .into_any_element()
}

fn horizontal_slider_state(value: f32) -> gadget_horizontal_slider::HorizontalSliderState {
    let mut state = gadget_horizontal_slider::HorizontalSliderState::default();
    state.position =
        (state.min as f32 + (state.max - state.min) as f32 * value.clamp(0.0, 1.0)).round() as i32;
    state
}

fn vertical_slider_state(value: f32) -> gadget_vertical_slider::VerticalSliderState {
    let mut state = gadget_vertical_slider::VerticalSliderState::default();
    state.position =
        (state.min as f32 + (state.max - state.min) as f32 * value.clamp(0.0, 1.0)).round() as i32;
    state
}

fn progress_bar_state(value: f32) -> gadget_progress_bar::ProgressBarState {
    gadget_progress_bar::ProgressBarState::new((value.clamp(0.0, 1.0) * 100.0).round() as u8)
}

fn check_box_state(checked: bool) -> gadget_check_box::CheckBoxState {
    gadget_check_box::CheckBoxState::new(checked)
}

fn text_entry_state(value: &str) -> gadget_text_entry::TextEntryState {
    gadget_text_entry::TextEntryState {
        text: value.to_string(),
        secret_text: "*".repeat(value.chars().count()),
        ..Default::default()
    }
}

fn combo_box_state(selected_text: String, entries: &[&str]) -> gadget_combo_box::ComboBoxState {
    gadget_combo_box::ComboBoxState {
        selected_text,
        entries: entries.iter().map(|entry| (*entry).to_string()).collect(),
        ..Default::default()
    }
}

fn list_box_state(
    entries: Vec<String>,
    selected_row: Option<usize>,
) -> gadget_list_box::ListBoxState {
    gadget_list_box::ListBoxState {
        entries,
        selected_row,
        display_rows: 8,
        ..Default::default()
    }
}

fn map_select_list_box_state(
    state: &crate::gui::callbacks::menus::map_select_menu::MapSelectMenuPort,
) -> gadget_list_box::ListBoxState {
    list_box_state(
        state
            .maps
            .iter()
            .map(|map| {
                format!(
                    "{} · {} players · {}",
                    map.display_name,
                    map.player_count,
                    if map.official { "Official" } else { "User Map" }
                )
            })
            .collect(),
        state.selected_index,
    )
}

fn replay_list_box_state(
    state: &crate::gui::callbacks::menus::replay_menu::ReplayMenuPort,
) -> gadget_list_box::ListBoxState {
    list_box_state(
        state
            .entries
            .iter()
            .map(|entry| entry.display_label())
            .collect(),
        state.selected_index,
    )
}

fn lobby_players_list_box_state(state: &LanLobbyMenuPort) -> gadget_list_box::ListBoxState {
    list_box_state(
        state
            .players
            .iter()
            .map(|player| {
                format!(
                    "{} · {} · {} · {}",
                    player.name,
                    player.faction,
                    player.color,
                    if player.ready { "Ready" } else { "Waiting" }
                )
            })
            .collect(),
        state.selected_player,
    )
}

fn stored_accounts_list_box_state(state: &WolLoginMenuPort) -> gadget_list_box::ListBoxState {
    let selected = state
        .stored_logins
        .iter()
        .position(|login| login.email == state.email);

    list_box_state(
        state
            .stored_logins
            .iter()
            .map(|login| {
                format!(
                    "{} · {} nick(s) · password {}",
                    login.email,
                    login.nicks.len(),
                    if login.has_password {
                        "saved"
                    } else {
                        "not saved"
                    }
                )
            })
            .collect(),
        selected,
    )
}

fn pct(value: f32) -> i32 {
    (value.clamp(0.0, 1.0) * 100.0).round() as i32
}

fn static_text(label: &str, body: impl Into<String>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(label.to_string())
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(body.into()))
        .into_any_element()
}

fn command_list(label: &str, entries: Vec<String>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(label.to_string())
        .children(entries.into_iter().map(|entry| {
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(format!("• {entry}"))
        }))
        .into_any_element()
}
