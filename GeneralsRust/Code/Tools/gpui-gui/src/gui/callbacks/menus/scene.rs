use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::gadget;
use crate::gui::gadget::{
    gadget_check_box, gadget_combo_box, gadget_horizontal_slider, gadget_list_box,
    gadget_progress_bar, gadget_push_button, gadget_radio_button, gadget_static_text,
    gadget_tab_control, gadget_text_entry, gadget_vertical_slider,
};
use crate::gui::source_catalog::MenuScreenPort;

pub fn render_screen(screen: MenuScreenPort) -> AnyElement {
    match screen.key {
        "MainMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                div()
                    .flex()
                    .gap_2()
                    .children([
                        gadget_push_button::render_demo("Single Player"),
                        gadget_push_button::render_demo("Skirmish"),
                        gadget_push_button::render_demo("Multiplayer"),
                        gadget_push_button::render_demo("Options"),
                    ])
                    .into_any_element(),
                gadget_static_text::render_demo(
                    "Shell Root",
                    "Top-level navigation ported from callback-driven shell menus into composable GPUI controls.",
                ),
            ],
        ),
        "SinglePlayerMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                div()
                    .flex()
                    .gap_2()
                    .children([
                        gadget_push_button::render_demo("Campaign"),
                        gadget_push_button::render_demo("Challenge"),
                        gadget_push_button::render_demo("Load Game"),
                    ])
                    .into_any_element(),
                gadget_static_text::render_demo(
                    "Single Player Flow",
                    "Campaign and challenge entry points with save/load integration.",
                ),
            ],
        ),
        "OptionsMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_tab_control::render_demo(
                    &["Audio", "Video", "Controls", "Gameplay"],
                    "Video",
                ),
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(gadget_horizontal_slider::render_demo("Music Volume: 68%", 0.68))
                            .child(gadget_horizontal_slider::render_demo("FX Volume: 82%", 0.82))
                            .child(gadget_combo_box::render_demo("1920x1080"))
                            .child(gadget_text_entry::render_demo("PlayerName_01")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(gadget_check_box::render_demo("Use dynamic lights", true))
                            .child(gadget_check_box::render_demo("Enable unit speech", true))
                            .child(gadget_check_box::render_demo("Show subtitles", false))
                            .child(gadget_vertical_slider::render_demo(0.5)),
                    )
                    .into_any_element(),
            ],
        ),
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
        "MapSelectMenu" | "SkirmishMapSelectMenu" | "LanMapSelectMenu" | "WOLMapSelectMenu" => {
            screen_frame(
                screen.title,
                screen.summary,
                vec![
                    gadget_list_box::render_demo(
                        &[
                            "Tournament Desert",
                            "Defcon 6",
                            "Forgotten Forest",
                            "Twilight Flame",
                        ],
                        "Tournament Desert",
                    ),
                    div()
                        .flex()
                        .gap_4()
                        .children([
                            gadget_combo_box::render_demo("2 Players"),
                            gadget_combo_box::render_demo("Balanced"),
                            gadget_check_box::render_demo("Official maps only", true),
                        ])
                        .into_any_element(),
                ],
            )
        }
        "ReplayMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_list_box::render_demo(
                    &["ZHFinal.rep", "LadderMatch.rep", "ChallengeRun.rep"],
                    "ZHFinal.rep",
                ),
                gadget_progress_bar::render_demo("Replay timeline", 0.44),
                div()
                    .flex()
                    .gap_2()
                    .children([
                        gadget_push_button::render_demo("Play"),
                        gadget_push_button::render_demo("Delete"),
                        gadget_push_button::render_demo("Rename"),
                    ])
                    .into_any_element(),
            ],
        ),
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
        "LanLobbyMenu" | "WOLLobbyMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                div()
                    .flex()
                    .gap_4()
                    .child(gadget_list_box::render_demo(
                        &["bernardo", "ai_hard_2", "guest42", "observer01"],
                        "bernardo",
                    ))
                    .child(gadget_static_text::render_demo(
                        "Lobby Chat",
                        "gg gl hf\nready when you are\nswitching to china",
                    ))
                    .into_any_element(),
                gadget_text_entry::render_demo("Type message..."),
            ],
        ),
        "WOLLoginMenu" => screen_frame(
            screen.title,
            screen.summary,
            vec![
                gadget_text_entry::render_demo("Email / Nickname"),
                gadget_text_entry::render_demo("Password"),
                gadget_check_box::render_demo("Remember account", true),
            ],
        ),
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
