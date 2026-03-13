use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::gadget::{
    gadget_check_box, gadget_horizontal_slider, gadget_list_box, gadget_progress_bar,
    gadget_push_button, gadget_static_text,
};
use crate::gui::source_catalog::ControlBarPort;

pub fn render_port(port: &ControlBarPort) -> AnyElement {
    match port.record.cpp_relative_path {
        "ControlBar/ControlBarCommand.cpp" => panel(
            port.label,
            vec![
                gadget_static_text::render_demo(
                    "Command Metadata",
                    "AttackMove / Guard / Sell / SetRallyPoint",
                ),
                gadget_push_button::render_demo("Resolve Command"),
            ],
        ),
        "ControlBar/ControlBarCommandProcessing.cpp" => panel(
            port.label,
            vec![
                gadget_static_text::render_demo(
                    "Dispatch Gate",
                    "Checks targeting mode, queueability, and context mode before firing.",
                ),
                gadget_check_box::render_demo("Queued order", true),
            ],
        ),
        "ControlBar/ControlBarStructureInventory.cpp" => panel(
            port.label,
            vec![gadget_list_box::render_demo(
                &["Passenger 1", "Passenger 2", "Drone Slot"],
                "Passenger 1",
            )],
        ),
        "ControlBar/ControlBarUnderConstruction.cpp" => panel(
            port.label,
            vec![
                gadget_progress_bar::render_demo("Build progress", 0.66),
                gadget_check_box::render_demo("Lock commands", true),
            ],
        ),
        "ControlBar/ControlBarBeacon.cpp" => panel(
            port.label,
            vec![
                gadget_push_button::render_demo("Place Beacon"),
                gadget_push_button::render_demo("Delete Beacon"),
            ],
        ),
        "ControlBar/ControlBarMultiSelect.cpp" => panel(
            port.label,
            vec![gadget_list_box::render_demo(
                &["4x Crusader", "2x Ambulance", "1x Dozer"],
                "4x Crusader",
            )],
        ),
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
