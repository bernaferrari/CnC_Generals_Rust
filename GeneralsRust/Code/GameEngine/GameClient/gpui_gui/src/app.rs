use gpui::{
    div, prelude::*, px, rgb, size, AnyElement, App, Application, Bounds, Context, SharedString,
    Window, WindowBounds, WindowOptions,
};

use crate::gui::callbacks as callback_ports;
use crate::gui::callbacks::menus;
use crate::gui::control_bar as control_bar_ports;
use crate::gui::control_bar::control_bar as control_bar_core;
use crate::gui::control_bar::control_bar_command::CommandBarStatePort;
use crate::gui::control_bar::control_bar_multi_select::MultiSelectPort;
use crate::gui::gadget;
use crate::gui::game_window::GameWindowPort;
use crate::gui::game_window_manager::GameWindowManagerPort;
use crate::gui::shell::shell::ShellPort;
use crate::gui::source_catalog::MenuScreenPort;
use crate::gui::system_scene;
use crate::legacy;

pub fn run() -> anyhow::Result<()> {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1480.), px(940.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |_, cx| cx.new(|_| StandaloneGuiApp::new()),
        )
        .expect("failed to open GPUI GUI window");
        cx.activate(true);
    });

    Ok(())
}

struct StandaloneGuiApp {
    selected_menu_group: &'static str,
    selected_screen: &'static str,
    shell: ShellPort,
    window_manager: GameWindowManagerPort,
    command_bar: CommandBarStatePort,
}

impl StandaloneGuiApp {
    fn new() -> Self {
        let selected_menu_group = menu_groups().into_iter().next().unwrap_or("Shell");
        let selected_screen = menu_screens_in_group(selected_menu_group)
            .first()
            .map(|screen| screen.key)
            .unwrap_or("MainMenu");

        let mut window_manager = GameWindowManagerPort::default();
        window_manager.init();

        let mut shell = ShellPort::default();
        shell.init();
        shell.show_shell(false, &mut window_manager);
        if selected_screen != "MainMenu" {
            shell.push(
                screen_layout_filename(selected_screen),
                false,
                &mut window_manager,
            );
        }

        let mut command_bar = CommandBarStatePort::default();
        command_bar.buttons = MultiSelectPort::sample().visible_commands();

        Self {
            selected_menu_group,
            selected_screen,
            shell,
            window_manager,
            command_bar,
        }
    }

    fn set_group(&mut self, group: &'static str, cx: &mut Context<Self>) {
        self.selected_menu_group = group;
        if let Some(screen) = menu_screens_in_group(group).first() {
            self.set_screen(screen.key, cx);
        } else {
            cx.notify();
        }
    }

    fn set_screen(&mut self, screen: &'static str, cx: &mut Context<Self>) {
        self.selected_screen = screen;
        let filename = screen_layout_filename(screen);
        let top_filename = self.shell.top().map(|layout| layout.filename.as_str());
        if top_filename != Some(filename.as_str()) {
            self.shell.push(filename, false, &mut self.window_manager);
        }
        cx.notify();
    }

    fn pop_screen(&mut self, cx: &mut Context<Self>) {
        self.shell.pop_immediate();
        self.sync_selected_screen_from_stack();
        cx.notify();
    }

    fn sync_selected_screen_from_stack(&mut self) {
        if let Some(top) = self.shell.top() {
            if let Some(screen) = menus::ports()
                .iter()
                .find(|screen| screen_layout_filename(screen.key) == top.filename)
                .copied()
            {
                self.selected_screen = screen.key;
                self.selected_menu_group = screen.group;
            }
        }
    }

    fn current_screen(&self) -> Option<MenuScreenPort> {
        menus::ports()
            .iter()
            .find(|screen| screen.key == self.selected_screen)
            .copied()
    }

    fn render_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .gap_3()
            .child(section_title("Shell Stack"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        action_chip("Pop Screen", false)
                            .on_click(cx.listener(|this, _, _, cx| this.pop_screen(cx))),
                    )
                    .child(metric_box("Depth", self.shell.stack.len().to_string())),
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
                                .child(screen.filename.clone())
                        }),
                ),
            )
            .child(section_title("Menu Groups"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(menu_groups().into_iter().map(|group| {
                        let active = self.selected_menu_group == group;
                        action_chip(group, active)
                            .on_click(cx.listener(move |this, _, _, cx| this.set_group(group, cx)))
                    })),
            )
            .child(section_title("Screens"))
            .child(
                div().flex().flex_col().gap_1().children(
                    menu_screens_in_group(self.selected_menu_group)
                        .into_iter()
                        .map(|screen| {
                            let active = self.selected_screen == screen.key;
                            div()
                                .id(screen.key)
                                .p_2()
                                .rounded_md()
                                .border_1()
                                .border_color(if active { rgb(0xd1a65d) } else { rgb(0x22303f) })
                                .bg(if active { rgb(0x221b12) } else { rgb(0x101720) })
                                .cursor_pointer()
                                .child(
                                    div().flex().flex_col().child(screen.title).child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(0x8ea2b4))
                                            .child(screen.summary),
                                    ),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_screen(screen.key, cx)
                                }))
                        }),
                ),
            )
    }

    fn render_active_screen(&self) -> AnyElement {
        menus::render_screen(self.selected_screen)
    }

    fn render_mapping_panel(&self) -> impl IntoElement {
        let current_screen = self.current_screen();
        let lifecycle = legacy::screen_by_name(self.selected_screen);
        let current_layout = self.shell.top();

        div()
            .w(px(340.))
            .flex()
            .flex_col()
            .gap_3()
            .child(section_title("Active Mapping"))
            .child(section_card(
                "Screen",
                vec![
                    metric_box("Key", self.selected_screen.to_string()),
                    metric_box(
                        "C++ File",
                        current_screen
                            .map(|screen| screen.record.cpp_relative_path.to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                    ),
                    metric_box(
                        "Rust Module",
                        current_screen
                            .map(|screen| screen.record.rust_module_path.to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                    ),
                    metric_box(
                        "Layout",
                        current_layout
                            .map(|layout| layout.filename.clone())
                            .unwrap_or_else(|| "none".to_string()),
                    ),
                ],
            ))
            .child(section_card(
                "Lifecycle",
                vec![
                    metric_box(
                        "Init",
                        lifecycle
                            .map(|l| l.lifecycle.init)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "Update",
                        lifecycle
                            .map(|l| l.lifecycle.update)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "Shutdown",
                        lifecycle
                            .map(|l| l.lifecycle.shutdown)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "System",
                        lifecycle
                            .map(|l| l.lifecycle.system)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                    metric_box(
                        "Input",
                        lifecycle
                            .map(|l| l.lifecycle.input)
                            .unwrap_or(false)
                            .to_string(),
                    ),
                ],
            ))
            .child(section_card(
                "Layout State",
                vec![
                    metric_box(
                        "Windows",
                        current_layout
                            .map(|layout| layout.window_count().to_string())
                            .unwrap_or_else(|| "0".to_string()),
                    ),
                    metric_box(
                        "Init Runs",
                        current_layout
                            .map(|layout| layout.init_runs.to_string())
                            .unwrap_or_else(|| "0".to_string()),
                    ),
                    metric_box(
                        "Update Runs",
                        current_layout
                            .map(|layout| layout.update_runs.to_string())
                            .unwrap_or_else(|| "0".to_string()),
                    ),
                    metric_box(
                        "Shutdown Runs",
                        current_layout
                            .map(|layout| layout.shutdown_runs.to_string())
                            .unwrap_or_else(|| "0".to_string()),
                    ),
                ],
            ))
            .child(section_title("Windows"))
            .child(
                div().flex().flex_col().gap_2().children(
                    current_layout
                        .map(|layout| {
                            layout
                                .windows
                                .iter()
                                .map(render_layout_window)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                ),
            )
    }
}

impl Render for StandaloneGuiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                        div().flex().flex_col().child("Generals GPUI GUI").child(
                            div().text_sm().text_color(rgb(0x8ea2b4)).child(
                                "Standalone GPUI rewrite path for the legacy GameClient GUI",
                            ),
                        ),
                    )
                    .child(div().flex().gap_2().children([
                        metric_box("Menus", menus::ports().len().to_string()),
                        metric_box("Gadgets", gadget::ports().len().to_string()),
                        metric_box("Shell Depth", self.shell.stack.len().to_string()),
                    ])),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .p_3()
                    .gap_3()
                    .child(self.render_navigation(cx))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(self.render_active_screen())
                            .child(section_title("Core Systems"))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_3()
                                    .children(system_scene::render_system_cards()),
                            )
                            .child(section_title("Control Bar"))
                            .child(control_bar_core::render_command_strip(&self.command_bar))
                            .child(
                                div().flex().flex_wrap().gap_3().children(
                                    control_bar_ports::ports()
                                        .iter()
                                        .skip(1)
                                        .take(4)
                                        .map(control_bar_ports::render_port),
                                ),
                            )
                            .child(section_title("Callbacks"))
                            .child(
                                div().flex().flex_wrap().gap_3().children(
                                    callback_ports::ports()
                                        .iter()
                                        .take(4)
                                        .map(callback_ports::render_port),
                                ),
                            ),
                    )
                    .child(self.render_mapping_panel()),
            )
    }
}

fn render_gadget_card(port: &crate::gui::source_catalog::GadgetPort) -> impl IntoElement {
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
        .child(port.label)
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(port.record.cpp_relative_path),
        )
        .child(gadget::render_port(port))
}

fn render_layout_window(window: &GameWindowPort) -> impl IntoElement {
    div()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x233242))
        .bg(rgb(0x101720))
        .flex()
        .flex_col()
        .gap_1()
        .child(window.title.clone())
        .child(div().text_sm().text_color(rgb(0x8ea2b4)).child(format!(
            "id={} rect=({}, {}, {}, {})",
            window.id, window.rect.x, window.rect.y, window.rect.width, window.rect.height
        )))
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(window.tooltip.clone()),
        )
}

fn metric_box(label: impl Into<SharedString>, value: impl Into<SharedString>) -> impl IntoElement {
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
}

fn section_title(label: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(rgb(0xd1a65d))
        .child(label.into())
}

fn section_card<E: IntoElement>(
    title: impl Into<SharedString>,
    metrics: Vec<E>,
) -> impl IntoElement {
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

fn screen_layout_filename(screen: &str) -> String {
    format!("Menus/{screen}.wnd")
}

fn menu_groups() -> Vec<&'static str> {
    let mut groups = Vec::new();
    for screen in menus::ports() {
        if !groups.contains(&screen.group) {
            groups.push(screen.group);
        }
    }
    groups
}

fn menu_screens_in_group(group: &str) -> Vec<MenuScreenPort> {
    menus::ports()
        .iter()
        .filter(|screen| screen.group == group)
        .copied()
        .collect()
}
