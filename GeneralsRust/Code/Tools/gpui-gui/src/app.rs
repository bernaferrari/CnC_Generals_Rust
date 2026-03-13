use gpui::{
    div, prelude::*, px, rgb, size, AnyElement, App, Application, Bounds, Context, SharedString,
    Window, WindowBounds, WindowOptions,
};

use crate::legacy::{self, LegacyCppUnit, LegacyScreenDescriptor};
use crate::model::{
    CommandOption, GuiCommandType, LegacyCommandButton, ShellState, WindowLayoutState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviewPanel {
    SourceTree,
    Shell,
    ControlBar,
    Widgets,
    Screens,
}

pub fn run() -> anyhow::Result<()> {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1440.), px(920.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |_, cx| cx.new(|_| LegacyGuiExplorer::new()),
        )
        .expect("failed to open GPUI GUI window");
        cx.activate(true);
    });

    Ok(())
}

struct LegacyGuiExplorer {
    selected_group: &'static str,
    selected_unit_path: &'static str,
    selected_screen: &'static str,
    selected_panel: PreviewPanel,
    shell: ShellState,
    command_buttons: Vec<LegacyCommandButton>,
}

impl LegacyGuiExplorer {
    fn new() -> Self {
        let selected_group = legacy::groups().into_iter().next().unwrap_or("Core");
        let selected_unit_path = legacy::units_in_group(selected_group)
            .into_iter()
            .next()
            .map(|unit| unit.relative_path)
            .unwrap_or("");
        let selected_screen = legacy::LEGACY_SCREENS
            .first()
            .map(|screen| screen.name)
            .unwrap_or("MainMenu");

        Self {
            selected_group,
            selected_unit_path,
            selected_screen,
            selected_panel: PreviewPanel::Shell,
            shell: ShellState {
                stack: vec!["MainMenu", "OptionsMenu"],
            },
            command_buttons: vec![
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
            ],
        }
    }

    fn set_group(&mut self, group: &'static str, cx: &mut Context<Self>) {
        self.selected_group = group;
        if let Some(first) = legacy::units_in_group(group).first() {
            self.selected_unit_path = first.relative_path;
        }
        cx.notify();
    }

    fn set_unit(&mut self, path: &'static str, cx: &mut Context<Self>) {
        self.selected_unit_path = path;
        cx.notify();
    }

    fn set_panel(&mut self, panel: PreviewPanel, cx: &mut Context<Self>) {
        self.selected_panel = panel;
        cx.notify();
    }

    fn set_screen(&mut self, screen: &'static str, cx: &mut Context<Self>) {
        self.selected_screen = screen;
        cx.notify();
    }

    fn push_screen(&mut self, screen: &'static str, cx: &mut Context<Self>) {
        self.shell.stack.push(screen);
        self.selected_screen = screen;
        cx.notify();
    }

    fn pop_screen(&mut self, cx: &mut Context<Self>) {
        if self.shell.stack.len() > 1 {
            self.shell.stack.pop();
            if let Some(screen) = self.shell.stack.last().copied() {
                self.selected_screen = screen;
            }
            cx.notify();
        }
    }

    fn current_unit(&self) -> Option<&'static LegacyCppUnit> {
        legacy::unit_by_path(self.selected_unit_path)
    }

    fn current_screen(&self) -> Option<&'static LegacyScreenDescriptor> {
        legacy::screen_by_name(self.selected_screen)
    }

    fn render_shell_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let layout = WindowLayoutState::preview_for(self.selected_screen);
        let quick_push = ["MainMenu", "SinglePlayerMenu", "OptionsMenu", "ReplayMenu"];

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children(quick_push.into_iter().map(|screen| {
                        let active = self.selected_screen == screen;
                        action_chip(screen, active).on_click(
                            cx.listener(move |this, _, _, cx| this.push_screen(screen, cx)),
                        )
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        action_chip("Pop Screen", false)
                            .on_click(cx.listener(|this, _, _, cx| this.pop_screen(cx))),
                    )
                    .child(metric_box(
                        "Shell Depth",
                        self.shell.stack.len().to_string(),
                    )),
            )
            .child(
                div().flex().flex_col().gap_2().children(
                    self.shell
                        .stack
                        .iter()
                        .rev()
                        .enumerate()
                        .map(|(ix, screen)| {
                            div()
                                .id(("shell-stack", ix))
                                .p_2()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(0x273645))
                                .bg(if ix == 0 {
                                    rgb(0x18232f)
                                } else {
                                    rgb(0x111922)
                                })
                                .child(format!("#{ix} {screen}"))
                        }),
                ),
            )
            .child(section_card(
                "Window Layout",
                vec![
                    metric_box("Source", layout.filename.clone()),
                    metric_box("Window Count", layout.windows.len().to_string()),
                    metric_box("Hidden", layout.hidden.to_string()),
                ],
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(layout.windows.into_iter().map(render_layout_window)),
            )
            .into_any_element()
    }

    fn render_control_bar_preview(&self) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().gap_2().children([
                metric_box("Credits", "$6,800".to_string()),
                metric_box("Power", "+153 / -128".to_string()),
                metric_box("Generals Points", "2".to_string()),
                metric_box("Idle Workers", "1".to_string()),
            ]))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .child(
                        div()
                            .w(px(320.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(command_bar_panel(
                                "Production Queue",
                                &[
                                    ("Scorpion Tank", 0.78_f32),
                                    ("Battle Bus", 0.31_f32),
                                    ("Demo Upgrade", 0.52_f32),
                                ],
                            )),
                    )
                    .child(
                        div().flex_1().flex().flex_col().gap_2().child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .children(self.command_buttons.iter().map(render_command_button)),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn render_widget_preview(&self) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_title("Gadget Ports"))
            .child(div().flex().flex_wrap().gap_3().children([
                widget_card("Push Button", "GadgetPushButton.cpp", render_push_button()),
                widget_card("Check Box", "GadgetCheckBox.cpp", render_checkbox()),
                widget_card(
                    "Radio Button",
                    "GadgetRadioButton.cpp",
                    render_radio_group(),
                ),
                widget_card(
                    "Horizontal Slider",
                    "GadgetHorizontalSlider.cpp",
                    render_slider(),
                ),
                widget_card("List Box", "GadgetListBox.cpp", render_listbox()),
                widget_card("Progress Bar", "GadgetProgressBar.cpp", render_progress()),
                widget_card("Text Entry", "GadgetTextEntry.cpp", render_text_entry()),
                widget_card("Tab Control", "GadgetTabControl.cpp", render_tab_control()),
            ]))
            .into_any_element()
    }

    fn render_source_preview(&self) -> AnyElement {
        let selected = self.current_unit();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_title("Legacy Source Map"))
            .child(section_card(
                "Selected Unit",
                vec![
                    metric_box(
                        "Relative Path",
                        selected
                            .map(|unit| unit.relative_path.to_string())
                            .unwrap_or_else(|| "None".to_string()),
                    ),
                    metric_box(
                        "Module",
                        selected
                            .map(|unit| unit.module_name.to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                    ),
                    metric_box(
                        "Category",
                        selected
                            .map(|unit| unit.category.to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                    ),
                ],
            ))
            .child(section_card(
                "Coverage",
                vec![
                    metric_box("C++ Units", legacy::LEGACY_CPP_UNITS.len().to_string()),
                    metric_box("Callback Screens", legacy::LEGACY_SCREENS.len().to_string()),
                    metric_box("Current Group", self.selected_group.to_string()),
                ],
            ))
            .into_any_element()
    }

    fn render_screens_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.current_screen();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().flex_wrap().gap_2().children(
                legacy::LEGACY_SCREENS.iter().take(20).map(|screen| {
                    let active = screen.name == self.selected_screen;
                    action_chip(screen.name, active).on_click(
                        cx.listener(move |this, _, _, cx| this.set_screen(screen.name, cx)),
                    )
                }),
            ))
            .child(section_card(
                "Lifecycle",
                vec![
                    metric_box(
                        "Init",
                        current
                            .map(|screen| screen.lifecycle.init)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "Update",
                        current
                            .map(|screen| screen.lifecycle.update)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "Shutdown",
                        current
                            .map(|screen| screen.lifecycle.shutdown)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "System",
                        current
                            .map(|screen| screen.lifecycle.system)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "Input",
                        current
                            .map(|screen| screen.lifecycle.input)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                ],
            ))
            .child(render_screen_mock(self.selected_screen))
            .into_any_element()
    }

    fn render_center_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.selected_panel {
            PreviewPanel::SourceTree => self.render_source_preview(),
            PreviewPanel::Shell => self.render_shell_preview(cx),
            PreviewPanel::ControlBar => self.render_control_bar_preview(),
            PreviewPanel::Widgets => self.render_widget_preview(),
            PreviewPanel::Screens => self.render_screens_preview(cx),
        }
    }
}

impl Render for LegacyGuiExplorer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let groups = legacy::groups();
        let units = legacy::units_in_group(self.selected_group);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x091017))
            .text_color(rgb(0xe7ecf1))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(0x16202c))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child("GeneralsMD GameClient GUI")
                            .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(
                                "Standalone GPUI port scaffold driven directly from the C++ tree",
                            )),
                    )
                    .child(div().flex().gap_2().children([
                        metric_box("Units", legacy::LEGACY_CPP_UNITS.len().to_string()),
                        metric_box("Screens", legacy::LEGACY_SCREENS.len().to_string()),
                        metric_box("Stack", self.shell.stack.len().to_string()),
                    ])),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .p_3()
                    .gap_3()
                    .child(
                        div()
                            .w(px(310.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(section_title("Source Groups"))
                            .child(div().flex().flex_wrap().gap_2().children(
                                groups.into_iter().map(|group| {
                                    let active = self.selected_group == group;
                                    action_chip(group, active).on_click(
                                        cx.listener(move |this, _, _, cx| {
                                            this.set_group(group, cx)
                                        }),
                                    )
                                }),
                            ))
                            .child(section_title("C++ Units"))
                            .child(div().flex().flex_col().gap_1().children(
                                units.into_iter().map(|unit| {
                                    let active = self.selected_unit_path == unit.relative_path;
                                    div()
                                        .id(unit.relative_path)
                                        .p_2()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(if active {
                                            rgb(0xd1a65d)
                                        } else {
                                            rgb(0x22303f)
                                        })
                                        .bg(if active { rgb(0x221b12) } else { rgb(0x101720) })
                                        .cursor_pointer()
                                        .child(
                                            div().flex().flex_col().child(unit.display_name).child(
                                                div()
                                                    .text_sm()
                                                    .text_color(rgb(0x8ea2b4))
                                                    .child(unit.relative_path),
                                            ),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.set_unit(unit.relative_path, cx)
                                        }))
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div().flex().gap_2().children([
                                    panel_chip(
                                        "Source",
                                        self.selected_panel == PreviewPanel::SourceTree,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.set_panel(PreviewPanel::SourceTree, cx)
                                        },
                                    )),
                                    panel_chip("Shell", self.selected_panel == PreviewPanel::Shell)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_panel(PreviewPanel::Shell, cx)
                                        })),
                                    panel_chip(
                                        "Control Bar",
                                        self.selected_panel == PreviewPanel::ControlBar,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.set_panel(PreviewPanel::ControlBar, cx)
                                        },
                                    )),
                                    panel_chip(
                                        "Widgets",
                                        self.selected_panel == PreviewPanel::Widgets,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| this.set_panel(PreviewPanel::Widgets, cx),
                                    )),
                                    panel_chip(
                                        "Screens",
                                        self.selected_panel == PreviewPanel::Screens,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| this.set_panel(PreviewPanel::Screens, cx),
                                    )),
                                ]),
                            )
                            .child(self.render_center_panel(cx)),
                    )
                    .child(
                        div()
                            .w(px(320.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(section_title("Selection"))
                            .child(section_card(
                                "Source Detail",
                                vec![
                                    metric_box("Group", self.selected_group.to_string()),
                                    metric_box("Path", self.selected_unit_path.to_string()),
                                    metric_box("Screen", self.selected_screen.to_string()),
                                ],
                            ))
                            .child(section_card(
                                "Legacy Mapping",
                                vec![
                                    metric_box(
                                        "Active Unit",
                                        self.current_unit()
                                            .map(|unit| unit.display_name.to_string())
                                            .unwrap_or_else(|| "None".to_string()),
                                    ),
                                    metric_box(
                                        "Category",
                                        self.current_unit()
                                            .map(|unit| unit.category.to_string())
                                            .unwrap_or_else(|| "unknown".to_string()),
                                    ),
                                    metric_box(
                                        "Lifecycle Group",
                                        self.current_screen()
                                            .map(|screen| screen.group.to_string())
                                            .unwrap_or_else(|| "unknown".to_string()),
                                    ),
                                ],
                            )),
                    ),
            )
    }
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

fn section_title(label: impl Into<SharedString>) -> AnyElement {
    div()
        .text_sm()
        .text_color(rgb(0xd1a65d))
        .child(label.into())
        .into_any_element()
}

fn section_card(title: impl Into<SharedString>, metrics: Vec<AnyElement>) -> AnyElement {
    div()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x0e1620))
        .flex()
        .flex_col()
        .gap_2()
        .child(title.into())
        .children(metrics)
        .into_any_element()
}

fn action_chip(label: &'static str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(if active { rgb(0xd1a65d) } else { rgb(0x22303f) })
        .bg(if active { rgb(0x221b12) } else { rgb(0x101720) })
        .cursor_pointer()
        .child(label)
}

fn panel_chip(label: &'static str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(if active { rgb(0x9fb3c8) } else { rgb(0x22303f) })
        .bg(if active { rgb(0x162331) } else { rgb(0x0f1720) })
        .cursor_pointer()
        .child(label)
}

fn render_layout_window(window: crate::model::LegacyWindowNode) -> AnyElement {
    div()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x233242))
        .bg(rgb(0x101720))
        .flex()
        .flex_col()
        .gap_1()
        .child(window.title)
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(format!(
            "id={} rect=({}, {}, {}, {})",
            window.id, window.rect.x, window.rect.y, window.rect.width, window.rect.height
        )))
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(window.tooltip),
        )
        .into_any_element()
}

fn command_bar_panel(title: &'static str, entries: &[(&'static str, f32)]) -> AnyElement {
    div()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x233242))
        .bg(rgb(0x101720))
        .flex()
        .flex_col()
        .gap_2()
        .child(title)
        .children(entries.iter().map(|(label, progress)| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(*label))
                .child(progress_bar(*progress, rgb(0xd1a65d)))
        }))
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
        .into_any_element()
}

fn widget_card(title: &'static str, source: &'static str, widget: AnyElement) -> AnyElement {
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
        .child(title)
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(source))
        .child(widget)
        .into_any_element()
}

fn render_push_button() -> AnyElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0xd1a65d))
        .bg(rgb(0x1f1910))
        .child("Launch")
        .into_any_element()
}

fn render_checkbox() -> AnyElement {
    div()
        .flex()
        .gap_2()
        .items_center()
        .child(
            div()
                .size(px(18.))
                .rounded_sm()
                .border_1()
                .border_color(rgb(0x8dc0ff))
                .bg(rgb(0x162331))
                .child("X"),
        )
        .child("Enable subtitles")
        .into_any_element()
}

fn render_radio_group() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(["Low", "Medium", "High"].into_iter().map(|label| {
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .size(px(16.))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x8dc0ff))
                        .bg(if label == "High" {
                            rgb(0x32567c)
                        } else {
                            rgb(0x101720)
                        }),
                )
                .child(label)
        }))
        .into_any_element()
}

fn render_slider() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .h(px(8.))
                .rounded_full()
                .bg(rgb(0x1f2a35))
                .child(div().w(px(112.)).h(px(8.)).rounded_full().bg(rgb(0x69d18a))),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child("Volume: 70%"),
        )
        .into_any_element()
}

fn render_listbox() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(
            ["Tournament Desert", "Forgotten Forest", "Defcon 6"]
                .into_iter()
                .map(|label| {
                    div()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(if label == "Tournament Desert" {
                            rgb(0x223347)
                        } else {
                            rgb(0x101720)
                        })
                        .child(label)
                }),
        )
        .into_any_element()
}

fn render_progress() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(progress_bar(0.66, rgb(0xd88a44)))
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child("Build progress"),
        )
        .into_any_element()
}

fn render_text_entry() -> AnyElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x111922))
        .child("PlayerName_01")
        .into_any_element()
}

fn render_tab_control() -> AnyElement {
    div()
        .flex()
        .gap_1()
        .children(["General", "Units", "Support"].into_iter().map(|label| {
            div()
                .px_3()
                .py_1()
                .rounded_t_md()
                .border_1()
                .border_color(rgb(0x22303f))
                .bg(if label == "Units" {
                    rgb(0x18232f)
                } else {
                    rgb(0x101720)
                })
                .child(label)
        }))
        .into_any_element()
}

fn progress_bar(progress: f32, fill_color: gpui::Rgba) -> AnyElement {
    let width = 196.0_f32 * progress.clamp(0.0, 1.0);
    div()
        .h(px(10.))
        .rounded_full()
        .bg(rgb(0x1e2935))
        .child(div().w(px(width)).h(px(10.)).rounded_full().bg(fill_color))
        .into_any_element()
}

fn render_screen_mock(screen: &str) -> AnyElement {
    let (headline, subline, accent) = match screen {
        "MainMenu" => (
            "Main Menu",
            "Campaign, skirmish, multiplayer, options",
            rgb(0xd1a65d),
        ),
        "OptionsMenu" => ("Options", "Audio, video, controls, gameplay", rgb(0x8dc0ff)),
        "ReplayMenu" => (
            "Replay Browser",
            "Saved match list and playback controls",
            rgb(0x69d18a),
        ),
        "ScoreScreen" => (
            "Score Screen",
            "Match summary and performance breakdown",
            rgb(0xd88a44),
        ),
        "ChallengeMenu" => (
            "Challenge",
            "General selection and mission ladder",
            rgb(0xde6b5c),
        ),
        _ => (
            "Legacy Screen",
            "Callback-driven shell layout preview",
            rgb(0xa68cff),
        ),
    };

    div()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x0e1620))
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(headline)
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(accent)
                        .text_color(rgb(0x091017))
                        .child(screen.to_string()),
                ),
        )
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(subline))
        .child(div().flex().gap_2().children([
            metric_box("Layout File", format!("Data/English/{screen}.wnd")),
            metric_box("Window Manager", "GameWindowManager.cpp".to_string()),
            metric_box("Callback Owner", format!("{screen}System / {screen}Input")),
        ]))
        .into_any_element()
}
