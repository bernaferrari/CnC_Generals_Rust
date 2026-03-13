use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::callbacks::message_box::{MessageBoxButtonPort, MessageBoxStatePort};
use crate::gui::gadget::{
    gadget_check_box, gadget_horizontal_slider, gadget_list_box, gadget_progress_bar,
    gadget_push_button, gadget_static_text, gadget_text_entry,
};
use crate::gui::source_catalog::CallbackPort;

pub fn render_port(port: &CallbackPort) -> AnyElement {
    match port.record.cpp_relative_path {
        "GUICallbacks/ControlBarCallback.cpp" => callback_card(
            port.label,
            vec![
                gadget_static_text::render_demo(
                    "Command Routing",
                    "Dispatches command-button events into gameplay-side control bar handlers.",
                ),
                gadget_push_button::render_demo("Fire Callback"),
            ],
        ),
        "GUICallbacks/ControlBarPopupDescription.cpp" => callback_card(
            port.label,
            vec![
                gadget_static_text::render_demo(
                    "Tooltip Preview",
                    "Build Scorpion Tank\nFast anti-armor unit with upgrade hooks.",
                ),
                gadget_progress_bar::render_demo("Tooltip delay", 0.35),
            ],
        ),
        "GUICallbacks/Diplomacy.cpp" => callback_card(
            port.label,
            vec![
                gadget_list_box::render_demo(
                    &["USA - Allied", "China - Neutral", "GLA - Enemy"],
                    "USA - Allied",
                ),
                gadget_check_box::render_demo("Share resources", false),
            ],
        ),
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
                    gadget_check_box::render_demo("Accepts keyboard focus", wants_focus),
                    gadget_check_box::render_demo(
                        "Destroyed after selection",
                        message_box.destroyed,
                    ),
                ],
            )
        }
        "GUICallbacks/GeneralsExpPoints.cpp" => callback_card(
            port.label,
            vec![
                gadget_progress_bar::render_demo("Promotion progress", 0.58),
                gadget_static_text::render_demo("Current Rank", "General Rank 3"),
            ],
        ),
        "GUICallbacks/IMECandidate.cpp" => callback_card(
            port.label,
            vec![
                gadget_list_box::render_demo(
                    &["Candidate 1", "Candidate 2", "Candidate 3"],
                    "Candidate 1",
                ),
                gadget_text_entry::render_demo("IME composition"),
            ],
        ),
        "GUICallbacks/InGameChat.cpp" => callback_card(
            port.label,
            vec![
                gadget_static_text::render_demo(
                    "Chat Log",
                    "ally: need power\nbernardo: building supply\nenemy spotted east",
                ),
                gadget_text_entry::render_demo("Type team message..."),
            ],
        ),
        "GUICallbacks/InGamePopupMessage.cpp" => callback_card(
            port.label,
            vec![
                gadget_static_text::render_demo("Popup", "General promotion available"),
                gadget_progress_bar::render_demo("Fade lifetime", 0.72),
            ],
        ),
        "GUICallbacks/ReplayControls.cpp" => callback_card(
            port.label,
            vec![
                div()
                    .flex()
                    .gap_2()
                    .children([
                        gadget_push_button::render_demo("Play"),
                        gadget_push_button::render_demo("Pause"),
                        gadget_push_button::render_demo("2x"),
                    ])
                    .into_any_element(),
                gadget_horizontal_slider::render_demo("Replay position", 0.46),
            ],
        ),
        _ => callback_card(
            port.label,
            vec![gadget_static_text::render_demo(
                "Callback Surface",
                port.summary,
            )],
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
