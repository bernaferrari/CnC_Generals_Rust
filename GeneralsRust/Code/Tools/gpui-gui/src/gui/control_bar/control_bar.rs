use gpui::{div, prelude::*, px, rgb, AnyElement, SharedString};

use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};
use crate::model::{CommandOption, GuiCommandType, LegacyCommandButton};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBar.cpp",
    "crate::gui::control_bar::control_bar",
    "Control Bar",
    "Ports the top-level context-sensitive command interface and its command grid.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Control Bar Core",
    "Owns command-set population, UI dirtying, and shell/HUD coordination.",
);

pub fn demo_buttons() -> Vec<LegacyCommandButton> {
    vec![
        LegacyCommandButton {
            label: "Attack",
            command: GuiCommandType::AttackMove,
            options: CommandOption::NEED_TARGET_POS,
            progress: 0.0,
            enabled: true,
        },
        LegacyCommandButton {
            label: "Guard",
            command: GuiCommandType::Guard,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        },
        LegacyCommandButton {
            label: "Power",
            command: GuiCommandType::SpecialPower,
            options: CommandOption::NEED_SPECIAL_POWER_SCIENCE,
            progress: 0.42,
            enabled: true,
        },
        LegacyCommandButton {
            label: "Rally",
            command: GuiCommandType::SetRallyPoint,
            options: CommandOption::NEED_TARGET_POS,
            progress: 0.0,
            enabled: true,
        },
        LegacyCommandButton {
            label: "Upgrade",
            command: GuiCommandType::PlayerUpgrade,
            options: CommandOption::NEED_UPGRADE,
            progress: 0.65,
            enabled: true,
        },
        LegacyCommandButton {
            label: "Sell",
            command: GuiCommandType::Sell,
            options: CommandOption::empty(),
            progress: 0.0,
            enabled: true,
        },
        LegacyCommandButton {
            label: "Beacon",
            command: GuiCommandType::PlaceBeacon,
            options: CommandOption::CONTEXTMODE_COMMAND,
            progress: 0.0,
            enabled: true,
        },
        LegacyCommandButton {
            label: "Stop",
            command: GuiCommandType::Stop,
            options: CommandOption::empty(),
            progress: 0.0,
            enabled: true,
        },
    ]
}

pub fn render_command_strip(buttons: &[LegacyCommandButton]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().flex().gap_2().children([
            metric_box("Credits", "$6,800"),
            metric_box("Power", "+153 / -128"),
            metric_box("Generals Points", "2"),
            metric_box("Idle Workers", "1"),
        ]))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(buttons.iter().map(render_command_button)),
        )
        .into_any_element()
}

fn render_command_button(button: &LegacyCommandButton) -> AnyElement {
    div()
        .w(px(168.))
        .p_2()
        .rounded_lg()
        .border_1()
        .border_color(if button.enabled {
            rgb(0xd1a65d)
        } else {
            rgb(0x394552)
        })
        .bg(rgb(0x131c26))
        .flex()
        .flex_col()
        .gap_1()
        .child(button.label)
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(button.command.title()),
        )
        .child(progress_bar(button.progress, rgb(0x69d18a)))
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x6f8190))
                .child(format!("{:?}", button.options)),
        )
        .into_any_element()
}

fn metric_box(label: impl Into<SharedString>, value: impl Into<SharedString>) -> AnyElement {
    div()
        .p_2()
        .rounded_md()
        .bg(rgb(0x101720))
        .border_1()
        .border_color(rgb(0x233242))
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x8ea2b4))
                        .child(label.into()),
                )
                .child(value.into()),
        )
        .into_any_element()
}

fn progress_bar(progress: f32, fill_color: gpui::Rgba) -> AnyElement {
    let width = 152.0_f32 * progress.clamp(0.0, 1.0);
    div()
        .h(px(10.))
        .rounded_full()
        .bg(rgb(0x1e2935))
        .child(div().w(px(width)).h(px(10.)).rounded_full().bg(fill_color))
        .into_any_element()
}
