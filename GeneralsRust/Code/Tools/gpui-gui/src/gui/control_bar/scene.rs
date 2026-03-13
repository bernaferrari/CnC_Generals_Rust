use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::control_bar::control_bar_beacon::ControlBarBeaconPort;
use crate::gui::control_bar::control_bar_command::CommandBarStatePort;
use crate::gui::control_bar::control_bar_command_processing::{
    CommandDispatchPort, CommandDispatchStatusPort,
};
use crate::gui::control_bar::control_bar_multi_select::MultiSelectPort;
use crate::gui::control_bar::control_bar_observer::ControlBarObserverPort;
use crate::gui::control_bar::control_bar_ocl_timer::ControlBarOclTimerPort;
use crate::gui::control_bar::control_bar_print_positions::ControlBarPrintPositionsPort;
use crate::gui::control_bar::control_bar_resizer::ControlBarResizerPort;
use crate::gui::control_bar::control_bar_scheme::ControlBarSchemePort;
use crate::gui::control_bar::control_bar_structure_inventory::ControlBarStructureInventoryPort;
use crate::gui::control_bar::control_bar_under_construction::UnderConstructionPort;
use crate::gui::gadget::gadget_progress_bar;
use crate::gui::source_catalog::ControlBarPort;
use crate::model::{CommandOption, GuiCommandType, LegacyCommandButton};

pub fn render_port(port: &ControlBarPort) -> AnyElement {
    match port.record.cpp_relative_path {
        "ControlBar/ControlBarCommand.cpp" => render_command_panel(port.label),
        "ControlBar/ControlBarCommandProcessing.cpp" => render_dispatch_panel(port.label),
        "ControlBar/ControlBarStructureInventory.cpp" => {
            render_structure_inventory_panel(port.label)
        }
        "ControlBar/ControlBarUnderConstruction.cpp" => render_under_construction_panel(port.label),
        "ControlBar/ControlBarBeacon.cpp" => render_beacon_panel(port.label),
        "ControlBar/ControlBarMultiSelect.cpp" => render_multi_select_panel(port.label),
        "ControlBar/ControlBarObserver.cpp" => render_observer_panel(port.label),
        "ControlBar/ControlBarOCLTimer.cpp" => render_ocl_timer_panel(port.label),
        "ControlBar/ControlBarResizer.cpp" => render_resizer_panel(port.label),
        "ControlBar/ControlBarScheme.cpp" => render_scheme_panel(port.label),
        "ControlBar/ControlBarPrintPositions.cpp" => render_positions_panel(port.label),
        _ => panel(
            port.label,
            vec![static_text("Subsystem", port.summary.to_string())],
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
            static_bool(
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
            static_bool("Rally point visible", state.rally_point_visible),
            static_text("Cancel Command", state.cancel_command.label.to_string()),
        ],
    )
}

fn render_structure_inventory_panel(title: &str) -> AnyElement {
    let inventory = ControlBarStructureInventoryPort::sample();
    panel(
        title,
        vec![command_list(
            "Slots",
            inventory
                .slots
                .iter()
                .enumerate()
                .map(|(index, slot)| {
                    format!(
                        "{}{} ({}%, {})",
                        if inventory.selected_slot == Some(index) {
                            "* "
                        } else {
                            ""
                        },
                        slot.occupant_name,
                        slot.health_pct,
                        if slot.exiting { "exiting" } else { "holding" }
                    )
                })
                .collect(),
        )],
    )
}

fn render_beacon_panel(title: &str) -> AnyElement {
    let beacon = ControlBarBeaconPort::sample();
    panel(
        title,
        vec![
            static_bool("Targeting Active", beacon.targeting_active),
            static_text("Beacon Count", beacon.beacon_count.to_string()),
            static_text(
                "Selected Beacon",
                beacon.selected_beacon.unwrap_or_else(|| "None".to_string()),
            ),
        ],
    )
}

fn render_observer_panel(title: &str) -> AnyElement {
    let observer = ControlBarObserverPort::sample();
    let current = observer.current_player();
    panel(
        title,
        vec![
            command_list(
                "Players",
                observer
                    .players
                    .iter()
                    .map(|player| player.name.clone())
                    .collect(),
            ),
            static_text(
                "Look At",
                current
                    .map(|player| player.name.clone())
                    .unwrap_or_else(|| "No observer target".to_string()),
            ),
            static_text(
                "Stats",
                current
                    .map(|player| {
                        format!(
                            "{} units / {} buildings / {} killed / {} lost",
                            player.units, player.buildings, player.units_killed, player.units_lost
                        )
                    })
                    .unwrap_or_else(|| "No player selected".to_string()),
            ),
        ],
    )
}

fn render_ocl_timer_panel(title: &str) -> AnyElement {
    let timer = ControlBarOclTimerPort::sample();
    panel(
        title,
        vec![
            static_text("Timer", timer.timer_name.clone()),
            gadget_progress_bar::render_demo("Cooldown", timer.progress()),
            static_text("Remaining Frames", timer.remaining_frames.to_string()),
        ],
    )
}

fn render_resizer_panel(title: &str) -> AnyElement {
    let mut resizer = ControlBarResizerPort::sample();
    resizer.apply_stage_offset(24, -32);
    panel(
        title,
        vec![
            static_text(
                "Resolution",
                format!("{}x{}", resizer.screen_width, resizer.screen_height),
            ),
            static_text(
                "Position",
                format!("{}, {}", resizer.current_x, resizer.current_y),
            ),
        ],
    )
}

fn render_scheme_panel(title: &str) -> AnyElement {
    let scheme = ControlBarSchemePort::sample();
    panel(
        title,
        vec![
            static_text("Side", scheme.side),
            static_text("Right HUD", scheme.right_hud_image),
            static_text(
                "Borders",
                format!(
                    "command {} / build {} / action {}",
                    scheme.command_bar_border_color,
                    scheme.build_border_color,
                    scheme.action_border_color
                ),
            ),
            static_text("Beacon Button", scheme.beacon_button_image),
        ],
    )
}

fn render_positions_panel(title: &str) -> AnyElement {
    let positions = ControlBarPrintPositionsPort::sample();
    panel(
        title,
        vec![command_list(
            "Anchors",
            positions
                .anchors
                .iter()
                .map(|anchor| format!("{}=({}, {})", anchor.label, anchor.x, anchor.y))
                .collect(),
        )],
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

fn static_bool(label: &str, value: bool) -> AnyElement {
    static_text(label, if value { "Yes" } else { "No" }.to_string())
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
