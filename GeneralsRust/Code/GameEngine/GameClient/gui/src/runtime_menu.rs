use gpui::{
    div, point, prelude::*, px, rgb, size, AnyElement, App, Application, Bounds, Context, Window,
    WindowBounds, WindowKind, WindowOptions,
};
use std::path::PathBuf;

pub fn run_runtime_menu(ipc_path: PathBuf) -> anyhow::Result<()> {
    let placement = RuntimeMenuPlacement::from_env();
    Application::new().run(move |cx: &mut App| {
        let bounds = placement
            .map(|p| {
                Bounds::new(
                    point(px(p.x as f32), px(p.y as f32)),
                    size(px(p.width as f32), px(p.height as f32)),
                )
            })
            .unwrap_or_else(|| Bounds::centered(None, size(px(1100.0), px(700.0)), cx));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                // Keep the GPUI menu as a separate top-level surface above the game window.
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                ..Default::default()
            },
            |_, cx| cx.new(|_| RuntimeMenuApp::new(ipc_path.clone())),
        )
        .expect("failed to open GPUI runtime menu window");
        cx.activate(true);
    });

    Ok(())
}

struct RuntimeMenuApp {
    ipc_path: PathBuf,
    panel: RuntimeMenuPanel,
}

#[derive(Clone, Copy)]
struct RuntimeMenuPlacement {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl RuntimeMenuPlacement {
    fn from_env() -> Option<Self> {
        let x = std::env::var("GENERALS_GPUI_MENU_X").ok()?.parse().ok()?;
        let y = std::env::var("GENERALS_GPUI_MENU_Y").ok()?.parse().ok()?;
        let width = std::env::var("GENERALS_GPUI_MENU_WIDTH")
            .ok()?
            .parse()
            .ok()?;
        let height = std::env::var("GENERALS_GPUI_MENU_HEIGHT")
            .ok()?
            .parse()
            .ok()?;
        Some(Self {
            x,
            y,
            width,
            height,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeMenuPanel {
    Main,
    Options,
}

impl RuntimeMenuApp {
    fn new(ipc_path: PathBuf) -> Self {
        Self {
            ipc_path,
            panel: RuntimeMenuPanel::Main,
        }
    }

    fn emit_action_and_exit(&self, action: &str) {
        let _ = std::fs::write(&self.ipc_path, action.as_bytes());
        std::process::exit(0);
    }

    fn render_menu_button(
        &self,
        label: &'static str,
        action: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(label)
            .w(px(340.0))
            .h(px(56.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x7d6a45))
            .bg(rgb(0x1f2430))
            .text_color(rgb(0xf5e9d7))
            .text_lg()
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, _cx| this.emit_action_and_exit(action)))
            .into_any_element()
    }

    fn render_panel_button(
        &self,
        label: &'static str,
        panel: RuntimeMenuPanel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(label)
            .w(px(340.0))
            .h(px(56.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x4d5a6f))
            .bg(rgb(0x172230))
            .text_color(rgb(0xc9d8e7))
            .text_lg()
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.panel = panel;
                cx.notify();
            }))
            .into_any_element()
    }
}

impl Render for RuntimeMenuApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_body = match self.panel {
            RuntimeMenuPanel::Main => vec![
                self.render_menu_button("Single Player", "start_skirmish", cx),
                self.render_menu_button("Skirmish", "start_skirmish", cx),
                self.render_panel_button("Options", RuntimeMenuPanel::Options, cx),
                self.render_menu_button("Exit Game", "exit_game", cx),
            ],
            RuntimeMenuPanel::Options => vec![
                div()
                    .w(px(340.0))
                    .text_sm()
                    .text_color(rgb(0x9fb2c5))
                    .child("Video, audio, and controls options are being migrated to GPUI.")
                    .into_any_element(),
                self.render_panel_button("Back", RuntimeMenuPanel::Main, cx),
            ],
        };

        div()
            .size_full()
            .bg(rgb(0x080d13))
            .text_color(rgb(0xf5e9d7))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(520.0))
                            .p_6()
                            .rounded_xl()
                            .border_1()
                            .border_color(rgb(0x2f3e51))
                            .bg(rgb(0x111923))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .text_2xl()
                                    .text_color(rgb(0xf5d28f))
                                    .child("Command & Conquer: Generals Zero Hour"),
                            )
                            .child(div().text_sm().text_color(rgb(0x9fb2c5)).child(
                                match self.panel {
                                    RuntimeMenuPanel::Main => "GPUI runtime menu",
                                    RuntimeMenuPanel::Options => "Options",
                                },
                            ))
                            .children(menu_body),
                    ),
            )
    }
}
