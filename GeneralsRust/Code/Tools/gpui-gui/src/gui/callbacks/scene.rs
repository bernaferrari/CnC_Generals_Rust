use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::callbacks::control_bar_callback::ControlBarCallbackPort;
use crate::gui::callbacks::control_bar_popup_description::{
    ControlBarPopupDescriptionPort, TooltipSubjectPort,
};
use crate::gui::callbacks::diplomacy::DiplomacyPort;
use crate::gui::callbacks::generals_exp_points::GeneralsExpPointsPort;
use crate::gui::callbacks::ime_candidate::ImeCandidateWindowPort;
use crate::gui::callbacks::in_game_chat::{
    ChatParticipantPort, InGameChatPort, InGameChatTypePort,
};
use crate::gui::callbacks::in_game_popup_message::InGamePopupMessagePort;
use crate::gui::callbacks::message_box::{MessageBoxButtonPort, MessageBoxStatePort};
use crate::gui::callbacks::replay_controls::{ReplayControlsPort, ReplayPlaybackStatePort};
use crate::gui::ime_manager::ImeManagerPort;
use crate::gui::source_catalog::CallbackPort;

pub fn render_port(port: &CallbackPort) -> AnyElement {
    match port.record.cpp_relative_path {
        "GUICallbacks/ControlBarCallback.cpp" => callback_card(port.label, {
            let state = ControlBarCallbackPort::sample();
            vec![
                static_text(
                    "Focus",
                    if state.handle_input_focus(true) {
                        "Accepted".to_string()
                    } else {
                        "Ignored".to_string()
                    },
                ),
                static_text(
                    "Last Routed",
                    state
                        .last_routed
                        .map(|message| {
                            format!("{} -> {}", message.control_name, message.gameplay_handler)
                        })
                        .unwrap_or_else(|| "none".to_string()),
                ),
                command_list(
                    "Messages",
                    state
                        .routed_messages
                        .into_iter()
                        .map(|message| format!("{:?}: {}", message.message, message.control_name))
                        .collect(),
                ),
            ]
        }),
        "GUICallbacks/ControlBarPopupDescription.cpp" => callback_card(port.label, {
            let mut tooltip = ControlBarPopupDescriptionPort::default();
            let _ = tooltip.show_build_tooltip_layout(22, 350, 1000);
            let _ = tooltip.show_build_tooltip_layout(22, 350, 1400);
            tooltip.populate_command_tooltip(
                "Scorpion Tank",
                Some(600),
                "Fast anti-armor unit with upgrade hooks.",
                &["Arms Dealer"],
                Some("Not enough money to build"),
            );
            tooltip.populate_generic_tooltip(TooltipSubjectPort::PowerWindow, 153, 128);
            vec![
                static_text("Title", tooltip.content.name),
                static_text(
                    "Cost",
                    tooltip
                        .content
                        .cost
                        .unwrap_or_else(|| "No direct cost".to_string()),
                ),
                static_text("Description", tooltip.content.description),
                static_text("Height", tooltip.panel_height.to_string()),
            ]
        }),
        "GUICallbacks/Diplomacy.cpp" => callback_card(port.label, {
            let diplomacy = DiplomacyPort::sample();
            vec![
                command_list(
                    "Players",
                    diplomacy
                        .players
                        .iter()
                        .map(|player| {
                            format!(
                                "{} / {} / team {} / {}{}",
                                player.name,
                                player.side,
                                player.team,
                                player.relation.label(),
                                if player.muted { " / muted" } else { "" }
                            )
                        })
                        .collect(),
                ),
                command_list("Briefing", diplomacy.solo_briefing_lines),
            ]
        }),
        "GUICallbacks/ExtendedMessageBox.cpp" | "GUICallbacks/MessageBox.cpp" => {
            let mut message_box = MessageBoxStatePort::yes_no(
                "Overwrite Save",
                "Do you want to overwrite this save?",
            );
            let wants_focus = message_box.handle_input_focus(true);
            let _ = message_box.select(MessageBoxButtonPort::Yes);
            callback_card(
                port.label,
                vec![
                    static_text("Title", message_box.title),
                    static_text("Prompt", message_box.body),
                    static_text(
                        "Buttons",
                        message_box
                            .buttons
                            .iter()
                            .map(|button| button.label())
                            .collect::<Vec<_>>()
                            .join(" / "),
                    ),
                    static_bool("Accepts keyboard focus", wants_focus),
                    static_bool("Destroyed after selection", message_box.destroyed),
                ],
            )
        }
        "GUICallbacks/GeneralsExpPoints.cpp" => callback_card(port.label, {
            let points = GeneralsExpPointsPort::sample();
            vec![
                static_text(
                    "Current Rank",
                    format!("General Rank {}", points.current_rank),
                ),
                static_text(
                    "Points",
                    format!(
                        "{} earned / {} spent / {} available",
                        points.earned_points,
                        points.spent_points,
                        points.available_points()
                    ),
                ),
                static_text(
                    "Promotion Progress",
                    format!("{}%", points.progress_to_next_rank_pct),
                ),
            ]
        }),
        "GUICallbacks/IMECandidate.cpp" => callback_card(port.label, {
            let mut ime = ImeManagerPort::default();
            ime.update_candidate_list(
                vec![
                    "Candidate 1".to_string(),
                    "Candidate 2".to_string(),
                    "Candidate 3".to_string(),
                ],
                3,
                0,
                1,
            );
            let mut candidate_window = ImeCandidateWindowPort::default();
            candidate_window.on_create();
            candidate_window.sync_from_ime(&ime);
            vec![
                command_list(
                    "Candidates",
                    candidate_window
                        .rows
                        .iter()
                        .map(|row| {
                            if row.selected {
                                format!("{} {}", row.number_label, row.candidate)
                            } else {
                                format!("{} {}", row.number_label, row.candidate)
                            }
                        })
                        .collect(),
                ),
                static_text(
                    "Display String",
                    candidate_window.display_string_allocated.to_string(),
                ),
            ]
        }),
        "GUICallbacks/InGameChat.cpp" => callback_card(port.label, {
            let mut chat = InGameChatPort::default();
            let _ = chat.show(false, false, false);
            chat.set_chat_type(InGameChatTypePort::Allies, true);
            chat.current_text = "ally hold center".to_string();
            let dispatch = chat.submit_message(
                &[
                    ChatParticipantPort {
                        slot: 0,
                        active: true,
                        muted: false,
                        allied_with_local: true,
                    },
                    ChatParticipantPort {
                        slot: 1,
                        active: true,
                        muted: false,
                        allied_with_local: true,
                    },
                    ChatParticipantPort {
                        slot: 2,
                        active: true,
                        muted: false,
                        allied_with_local: false,
                    },
                ],
                0,
                true,
                0,
                0,
            );
            vec![
                static_text("Channel", chat.chat_type_label),
                static_text(
                    "Last Dispatch",
                    dispatch
                        .map(|dispatch| {
                            format!(
                                "mask={:#05b} {}",
                                dispatch.player_mask, dispatch.filtered_message
                            )
                        })
                        .unwrap_or_else(|| "none".to_string()),
                ),
                static_text("Entry", "Type team message..."),
            ]
        }),
        "GUICallbacks/InGamePopupMessage.cpp" => callback_card(port.label, {
            let popup = InGamePopupMessagePort::sample();
            vec![
                static_text("Popup", popup.message.clone()),
                static_text("Fade Lifetime", format!("{:.0}%", popup.progress() * 100.0)),
            ]
        }),
        "GUICallbacks/ReplayControls.cpp" => callback_card(port.label, {
            let mut replay = ReplayControlsPort::default();
            replay.play();
            replay.set_speed(2);
            replay.seek(0.46);
            vec![
                static_text(
                    "Playback",
                    match replay.playback_state {
                        ReplayPlaybackStatePort::Playing => "Playing".to_string(),
                        ReplayPlaybackStatePort::Paused => "Paused".to_string(),
                    },
                ),
                static_text("Speed", format!("{}x", replay.speed_multiplier)),
                static_text(
                    "Replay Position",
                    format!("{:.0}%", replay.timeline_position * 100.0),
                ),
            ]
        }),
        _ => callback_card(
            port.label,
            vec![static_text("Callback Surface", port.summary)],
        ),
    }
}

fn callback_card(title: &str, body: Vec<AnyElement>) -> AnyElement {
    div()
        .w(px(260.))
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x0e1620))
        .flex()
        .flex_col()
        .gap_2()
        .child(title.to_string())
        .children(body)
        .into_any_element()
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

fn static_bool(label: &str, value: bool) -> AnyElement {
    static_text(label, if value { "Yes" } else { "No" })
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
