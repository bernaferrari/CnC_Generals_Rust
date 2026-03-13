use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::control_bar::control_bar_command::CommandBarStatePort;
use crate::gui::control_bar::control_bar_command_processing::{
    CommandDispatchPort, CommandDispatchStatusPort,
};
use crate::gui::control_bar::control_bar_multi_select::MultiSelectPort;
use crate::gui::control_bar::control_bar_under_construction::UnderConstructionPort;
use crate::gui::gadget::{
    gadget_check_box, gadget_horizontal_slider, gadget_list_box, gadget_progress_bar,
    gadget_push_button, gadget_static_text,
};
use crate::gui::source_catalog::ControlBarPort;
use crate::model::{CommandOption, GuiCommandType, LegacyCommandButton};

pub fn render_port(port: &ControlBarPort) -> AnyElement {
    match port.record.cpp_relative_path {
        "ControlBar/ControlBarCommand.cpp" => render_command_panel(port.label),
        "ControlBar/ControlBarCommandProcessing.cpp" => render_dispatch_panel(port.label),
        "ControlBar/ControlBarStructureInventory.cpp" => panel(
            port.label,
            vec![gadget_list_box::render_demo(
                &["Passenger 1", "Passenger 2", "Drone Slot"],
                "Passenger 1",
            )],
        ),
        "ControlBar/ControlBarUnderConstruction.cpp" => render_under_construction_panel(port.label),
        "ControlBar/ControlBarBeacon.cpp" => panel(
            port.label,
            vec![
                gadget_push_button::render_demo("Place Beacon"),
                gadget_push_button::render_demo("Delete Beacon"),
            ],
        ),
        "ControlBar/ControlBarMultiSelect.cpp" => render_multi_select_panel(port.label),
        "ControlBar/ControlBarObserver.cpp" => panel(
            port.label,
            vec![gadget_static_text::render_demo(
                "Observer HUD",
                "Selection commands suppressed; camera and scoreboard remain active.",
            )],
        ),
        "ControlBar/ControlBarOCLTimer.cpp" => panel(
            port.label,
            vec![gadget_progress_bar::render_demo("OCL cooldown", 0.41)],
        ),
        "ControlBar/ControlBarResizer.cpp" => panel(
            port.label,
            vec![gadget_horizontal_slider::render_demo(
                "Anchoring blend",
                0.52,
            )],
        ),
        "ControlBar/ControlBarScheme.cpp" => panel(
            port.label,
            vec![gadget_static_text::render_demo(
                "Faction Theme",
                "USA / China / GLA art layers, colors, and overlays.",
            )],
        ),
        "ControlBar/ControlBarPrintPositions.cpp" => panel(
            port.label,
            vec![gadget_static_text::render_demo(
                "Debug Anchors",
                "ButtonGrid=(64,768) Radar=(1048,706) Money=(148,704)",
            )],
        ),
        _ => panel(
            port.label,
            vec![gadget_static_text::render_demo("Subsystem", port.summary)],
        ),
    }
}

fn render_command_panel(title: &str) -> AnyElement {
    let state = CommandBarStatePort::default();
    panel(
        title,
        vec![
            static_text(
                "Metrics",
                format!(
                    "${} | power +{} / -{} | generals {}",
                    state.metrics.credits,
                    state.metrics.power_produced,
                    state.metrics.power_consumed,
                    state.metrics.generals_points
                ),
            ),
            command_list(
                "Visible Commands",
                state
                    .buttons
                    .iter()
                    .map(|button| button.label.to_string())
                    .collect(),
            ),
        ],
    )
}

fn render_dispatch_panel(title: &str) -> AnyElement {
    let mut dispatch = CommandDispatchPort {
        queue_mode: true,
        ..Default::default()
    };
    let build_button = LegacyCommandButton {
        label: "Build Strategy Center",
        command: GuiCommandType::DozerConstruct,
        options: CommandOption::empty(),
        progress: 0.0,
        enabled: true,
    };
    let target_button = LegacyCommandButton {
        label: "Attack Move",
        command: GuiCommandType::AttackMove,
        options: CommandOption::NEED_TARGET_POS,
        progress: 0.0,
        enabled: true,
    };

    let build_status =
        dispatch.process_command_ui(&build_button, true, false, false, false, false, false);
    let target_status =
        dispatch.process_command_ui(&target_button, true, true, true, false, false, false);

    panel(
        title,
        vec![
            static_text("Build Status", format!("{build_status:?}")),
            static_text(
                "Last Message",
                dispatch
                    .last_message
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
            ),
            gadget_check_box::render_demo(
                "Targeting mode armed",
                target_status == CommandDispatchStatusPort::EnterTargetMode,
            ),
        ],
    )
}

fn render_multi_select_panel(title: &str) -> AnyElement {
    let multi_select = MultiSelectPort::sample();
    panel(
        title,
        vec![
            static_text(
                "Selection",
                format!(
                    "{} selected / {} actionable",
                    multi_select.selected_units, multi_select.actionable_units
                ),
            ),
            static_text(
                "Portrait",
                multi_select
                    .portrait_name
                    .clone()
                    .unwrap_or_else(|| "Mixed portraits".to_string()),
            ),
            command_list(
                "Common Commands",
                multi_select
                    .visible_commands()
                    .into_iter()
                    .map(|command| {
                        let status = if command.enabled { "ready" } else { "blocked" };
                        format!("{} ({status})", command.label)
                    })
                    .collect(),
            ),
        ],
    )
}

fn render_under_construction_panel(title: &str) -> AnyElement {
    let state = UnderConstructionPort::default();
    panel(
        title,
        vec![
            static_text("Description", state.description_text.clone()),
            gadget_progress_bar::render_demo(
                "Build progress",
                state.construction_percent as f32 / 100.0,
            ),
            gadget_check_box::render_demo("Rally point visible", state.rally_point_visible),
            static_text("Cancel Command", state.cancel_command.label.to_string()),
        ],
    )
}

fn panel(title: &str, body: Vec<AnyElement>) -> AnyElement {
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

fn static_text(label: &str, body: String) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(label.to_string())
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(body))
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
