use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::callbacks::menus::lan_lobby_menu::LanLobbyMenuPort;
use crate::gui::callbacks::menus::main_menu::{
    CampaignSidePort, MainMenuDropdownPort, MainMenuPort,
};
use crate::gui::callbacks::menus::map_select_menu::MapSelectMenuPort;
use crate::gui::callbacks::menus::options_menu::{OptionsMenuPort, OptionsTabPort};
use crate::gui::callbacks::menus::replay_menu::{ReplayMenuPort, ReplayPromptPort};
use crate::gui::callbacks::menus::single_player_menu::SinglePlayerMenuPort;
use crate::gui::callbacks::menus::wol_login_menu::WolLoginMenuPort;
use crate::gui::gadget;
use crate::gui::gadget::{
    gadget_check_box, gadget_combo_box, gadget_horizontal_slider, gadget_list_box,
    gadget_progress_bar, gadget_push_button, gadget_radio_button, gadget_static_text,
    gadget_tab_control, gadget_text_entry, gadget_vertical_slider,
};
use crate::gui::source_catalog::MenuScreenPort;

pub fn render_screen(screen: MenuScreenPort) -> AnyElement {
    match screen.key {
        "MainMenu" => render_main_menu_screen(screen),
        "SinglePlayerMenu" => render_single_player_screen(screen),
        "OptionsMenu" => render_options_screen(screen),
        "KeyboardOptionsMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_list_box::render_demo(
                    &["Attack Move", "Force Fire", "Select All War Factories", "Guard"],
                    "Attack Move",
                ),
                gadget_text_entry::render_demo("A"),
                gadget_check_box::render_demo("Allow conflicting binds", false),
            ],
        ),
        "MapSelectMenu" | "SkirmishMapSelectMenu" | "LanMapSelectMenu" | "WOLMapSelectMenu" =>
            render_map_select_screen(screen),
        "ReplayMenu" => render_replay_menu_screen(screen),
        "ChallengeMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_radio_button::render_demo(
                    &[
                        "General Leang",
                        "General Kwai",
                        "General Alexander",
                        "General Townes",
                    ],
                    "General Alexander",
                ),
                gadget_progress_bar::render_demo("Challenge ladder", 0.37),
            ],
        ),
        "SkirmishGameOptionsMenu" | "LanGameOptionsMenu" | "WOLGameSetupMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_tab_control::render_demo(&["Map", "Army", "Rules"], "Army"),
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(gadget_combo_box::render_demo("USA"))
                            .child(gadget_text_entry::render_demo("AI Slot 2"))
                            .child(gadget_check_box::render_demo("Superweapons", false))
                            .child(gadget_check_box::render_demo("Crates", true)),
                    )
                    .child(gadget_radio_button::render_demo(
                        &["Easy", "Medium", "Hard"],
                        "Hard",
                    ))
                    .into_any_element(),
            ],
        ),
        "LanLobbyMenu" | "WOLLobbyMenu" => render_lobby_screen(screen),
        "WOLLoginMenu" => render_wol_login_screen(screen),
        "WOLWelcomeMenu" | "WOLStatusMenu" | "WOLLadderScreen" | "WOLQuickMatchMenu"
        | "WOLMessageWindow" | "WOLBuddyOverlay" | "WOLCustomScoreScreen"
        | "WOLQMScoreScreen" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_static_text::render_demo(
                    "Online Surface",
                    "This GPUI screen replaces the legacy callback screen with a clearer compositional layer while preserving subsystem boundaries.",
                ),
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(gadget::ports().iter().take(4).map(render_gadget_card))
                    .into_any_element(),
            ],
        ),
        "PopupCommunicator"
        | "PopupHostGame"
        | "PopupJoinGame"
        | "PopupLadderSelect"
        | "PopupPlayerInfo"
        | "PopupReplay"
        | "SaveLoadMenu"
        | "DifficultySelect"
        | "DisconnectWindow"
        | "EstablishConnectionsWindow"
        | "GameInfoWindow"
        | "WOLLocaleSelect" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                popup_surface(screen),
                div()
                    .flex()
                    .gap_2()
                    .children([
                        gadget_push_button::render_demo("Confirm"),
                        gadget_push_button::render_demo("Cancel"),
                    ])
                    .into_any_element(),
            ],
        ),
        "ScoreScreen" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                div()
                    .flex()
                    .gap_2()
                    .children([
                        stat_card("Units Lost", "54"),
                        stat_card("Units Destroyed", "88"),
                        stat_card("Structures", "12"),
                        stat_card("Cash Float", "$3,412"),
                    ])
                    .into_any_element(),
                gadget_progress_bar::render_demo("Overall rating", 0.74),
            ],
        ),
        "CreditsMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![gadget_static_text::render_demo(
                "Credits Roll",
                "Engineering\nDesign\nAudio\nQuality Assurance\nCommunity",
            )],
        ),
        _ => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_static_text::render_demo(
                    "Mapped Screen",
                    "This callback file has a dedicated Rust module and is now routed through the shared GPUI menu scene system.",
                ),
                default_scene_by_group(screen.group),
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
                        .child(gadget_horizontal_slider::render_demo(
                            &format!("Music Volume: {}%", pct(state.music_volume)),
                            state.music_volume,
                        ))
                        .child(gadget_horizontal_slider::render_demo(
                            &format!("FX Volume: {}%", pct(state.sfx_volume)),
                            state.sfx_volume,
                        ))
                        .child(gadget_horizontal_slider::render_demo(
                            &format!("Voice Volume: {}%", pct(state.voice_volume)),
                            state.voice_volume,
                        ))
                        .child(gadget_combo_box::render_demo(&format!(
                            "{}x{}",
                            state.resolution.0, state.resolution.1
                        )))
                        .child(gadget_combo_box::render_demo(&format!(
                            "{}x AA",
                            state.anti_aliasing
                        )))
                        .child(gadget_horizontal_slider::render_demo(
                            &format!("Scroll Speed: {}%", pct(state.scroll_speed)),
                            state.scroll_speed,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(gadget_check_box::render_demo(
                            "Retaliation mode",
                            state.retaliation_mode,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Double-click attack move",
                            state.double_click_attack_move,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Alternate mouse",
                            state.alternate_mouse,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Language filter",
                            state.language_filter,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Use camera in replays",
                            state.use_camera_in_replays,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Save camera in replays",
                            state.save_camera_in_replays,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Cloud shadows",
                            state.cloud_shadows,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Ground lighting",
                            state.ground_lighting,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Heat effects",
                            state.heat_effects,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Unlock FPS",
                            state.unlock_fps,
                        ))
                        .child(gadget_vertical_slider::render_demo(state.gamma)),
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
                    .children(state.entries.iter().enumerate().map(|(index, entry)| {
                        replay_row(index == state.selected_index.unwrap_or(usize::MAX), entry)
                    })),
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
    let mut state = MapSelectMenuPort::sample();
    let _ = state.select_map(1);
    state.set_difficulty(crate::gui::callbacks::menus::main_menu::GameDifficultyPort::Hard);

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(section_title("Available Maps"))
                        .children(state.maps.iter().enumerate().map(|(index, map)| {
                            let meta = format!(
                                "{} players · {}",
                                map.player_count,
                                if map.official { "Official" } else { "User Map" }
                            );
                            div()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(if state.selected_index == Some(index) {
                                    rgb(0xd1a65d)
                                } else {
                                    rgb(0x22303f)
                                })
                                .bg(if state.selected_index == Some(index) {
                                    rgb(0x1f1910)
                                } else {
                                    rgb(0x101720)
                                })
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(map.display_name.clone())
                                .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(meta))
                        }))
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
                        .child(gadget_check_box::render_demo(
                            "Solo maps",
                            state.show_solo_maps,
                        ))
                        .child(gadget_check_box::render_demo(
                            "Back requested",
                            state.pop_requested,
                        )),
                )
                .into_any_element(),
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
        ],
    )
}

fn render_lobby_screen(screen: MenuScreenPort) -> AnyElement {
    let mut state = LanLobbyMenuPort::sample();
    state.chat_entry = "ready when you are".to_string();
    let _ = state.send_chat();
    state.update();
    state.update();

    screen_frame(
        screen.title,
        screen.summary,
        vec![
            div()
                .flex()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(section_title("Players"))
                        .children(state.players.iter().enumerate().map(|(index, player)| {
                            selectable_row(
                                index == state.selected_player.unwrap_or(usize::MAX),
                                &player.name,
                                format!(
                                    "{} · {} · {}",
                                    player.faction,
                                    player.color,
                                    if player.ready { "Ready" } else { "Waiting" }
                                ),
                            )
                        }))
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
                        .child(gadget_text_entry::render_demo("ready when you are"))
                        .child(status_row(
                            "Transition",
                            state
                                .active_transition_group
                                .clone()
                                .unwrap_or_else(|| "LanLobbyMenuFade pending".to_string()),
                        )),
                )
                .into_any_element(),
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
        ],
    )
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
                    .children(state.stored_logins.iter().map(|login| {
                        selectable_row(
                            login.email == state.email,
                            &login.email,
                            format!(
                                "{} nick(s) · password {}",
                                login.nicks.len(),
                                if login.has_password {
                                    "saved"
                                } else {
                                    "not saved"
                                }
                            ),
                        )
                    }))
                    .into_any_element(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(status_row("Email", state.email.clone()))
                    .child(status_row("Nick", state.nick.clone()))
                    .child(gadget_check_box::render_demo(
                        "Remember account",
                        state.remember_account,
                    ))
                    .child(gadget_check_box::render_demo(
                        "Password stored",
                        state.password_present,
                    ))
                    .child(status_row("Status", state.status_message.clone())),
            )
            .into_any_element()],
    )
}

fn default_scene_by_group(group: &str) -> AnyElement {
    match group {
        "Popup" => div()
            .flex()
            .gap_2()
            .children([
                gadget_text_entry::render_demo("Input / selection field"),
                gadget_push_button::render_demo("Apply"),
            ])
            .into_any_element(),
        "LAN" => div()
            .flex()
            .gap_4()
            .child(gadget_list_box::render_demo(
                &["Local match", "Custom game", "Direct IP"],
                "Local match",
            ))
            .child(gadget_text_entry::render_demo("LAN player name"))
            .into_any_element(),
        "WOL" => div()
            .flex()
            .gap_4()
            .child(gadget_text_entry::render_demo("Online nick"))
            .child(gadget_combo_box::render_demo("Ranked 1v1"))
            .child(gadget_check_box::render_demo("Appear online", true))
            .into_any_element(),
        _ => div()
            .flex()
            .flex_wrap()
            .gap_3()
            .children(gadget::ports().iter().take(6).map(render_gadget_card))
            .into_any_element(),
    }
}

fn popup_surface(screen: MenuScreenPort) -> AnyElement {
    div()
        .max_w(px(520.))
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x35506b))
        .bg(rgb(0x101922))
        .flex()
        .flex_col()
        .gap_3()
        .child(screen.title.to_string())
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(screen.summary.to_string()),
        )
        .child(gadget_text_entry::render_demo("Input / selection field"))
        .into_any_element()
}

fn render_gadget_card(port: &crate::gui::source_catalog::GadgetPort) -> AnyElement {
    div()
        .w(px(240.))
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x0e1620))
        .flex()
        .flex_col()
        .gap_2()
        .child(port.label)
        .child(gadget::render_port(port))
        .into_any_element()
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
            gadget_check_box::render_demo(
                "USA recent save",
                state.selective_buttons.usa_recent_save,
            ),
            gadget_check_box::render_demo("USA load game", state.selective_buttons.usa_load_game),
            gadget_check_box::render_demo(
                "GLA recent save",
                state.selective_buttons.gla_recent_save,
            ),
            gadget_check_box::render_demo("GLA load game", state.selective_buttons.gla_load_game),
            gadget_check_box::render_demo(
                "China recent save",
                state.selective_buttons.china_recent_save,
            ),
            gadget_check_box::render_demo(
                "China load game",
                state.selective_buttons.china_load_game,
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
    div()
        .px_4()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(if active { rgb(0xd1a65d) } else { rgb(0x35506b) })
        .bg(if active { rgb(0x2a2011) } else { rgb(0x101720) })
        .child(label.to_string())
        .into_any_element()
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

fn selectable_row(selected: bool, label: &str, value: impl ToString) -> AnyElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(if selected {
            rgb(0xd1a65d)
        } else {
            rgb(0x22303f)
        })
        .bg(if selected {
            rgb(0x1f1910)
        } else {
            rgb(0x101720)
        })
        .flex()
        .flex_col()
        .gap_1()
        .child(label.to_string())
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(value.to_string()),
        )
        .into_any_element()
}

fn replay_row(
    selected: bool,
    entry: &crate::gui::callbacks::menus::replay_menu::ReplayEntryPort,
) -> AnyElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(if selected {
            rgb(0xd1a65d)
        } else {
            rgb(0x22303f)
        })
        .bg(if selected {
            rgb(0x1f1910)
        } else {
            rgb(0x101720)
        })
        .flex()
        .flex_col()
        .gap_1()
        .child(entry.display_label())
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(format!("{} · {}", entry.version, entry.replay_filename)),
        )
        .into_any_element()
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
