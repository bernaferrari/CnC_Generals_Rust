//! One borrowed native UI overlay, submitted after the scene on the same frame.
use game_client::core::subsystems::MouseHandle;
use game_client::display::display_fx::DisplayDrawFacts;
use game_client::gui::{
    ui_renderer::UIRenderer, window_draw_context::WindowDrawContext,
    window_manager::WindowManagerOwner,
};
use ww3d_renderer_3d::RendererResult;

pub(crate) type NativeUiOverlay<'a> = dyn FnMut(
        &mut ww3d_engine::RenderFrame,
        Option<&super::ui_radar::NativeRadarFrame>,
    ) -> RendererResult<()>
    + 'a;

/// This receiver cannot outlive the driving GameClient's renderer loan.
pub(crate) struct NativeUiDraw<'a> {
    renderer: &'a mut UIRenderer,
    windows: &'a WindowManagerOwner,
    display: DisplayDrawFacts,
    mouse: Option<MouseHandle>,
    ime: &'a game_client::gui::ime_manager::ImeManager,
    hints: Vec<game_client::gui::command_hints::CommandHint>,
    client_frame: u32,
    loading: bool,
    ui: Option<&'a mut game_client::core::subsystems::InGameUISubsystem>,
}
impl<'a> NativeUiDraw<'a> {
    pub(crate) fn new(
        renderer: &'a mut UIRenderer,
        windows: &'a WindowManagerOwner,
        display: DisplayDrawFacts,
        mouse: Option<MouseHandle>,
        ime: &'a game_client::gui::ime_manager::ImeManager,
    ) -> Self {
        Self {
            renderer,
            windows,
            display,
            mouse,
            ime,
            hints: Vec::new(),
            client_frame: 0,
            loading: false,
            ui: None,
        }
    }
    pub(crate) fn with_receiving_ui(
        mut self,
        ui: Option<&'a mut game_client::core::subsystems::InGameUISubsystem>,
    ) -> Self {
        self.ui = ui;
        self
    }
    pub(crate) fn for_load_screen(mut self) -> Self {
        self.loading = true;
        self
    }
    pub(crate) fn with_command_hints(
        mut self,
        hints: Vec<game_client::gui::command_hints::CommandHint>,
        client_frame: u32,
    ) -> Self {
        self.hints = hints;
        self.client_frame = client_frame;
        self
    }
    pub(crate) fn flush(
        &mut self,
        frame: &mut ww3d_engine::RenderFrame,
        radar: Option<&super::ui_radar::NativeRadarFrame>,
    ) -> RendererResult<()> {
        let shell_active = if self.loading {
            true
        } else {
            self.windows.shell_is_active().ok_or_else(|| {
                ww3d_renderer_3d::Error::InvalidOperation("native UI has no receiving Shell".into())
            })?
        };
        self.renderer.begin_overlay_frame();
        let mut cleanup = UiFrameCleanup::new(&mut *self.renderer);
        let renderer = &mut *cleanup.renderer;
        // C++ creates the ControlBar during gameplay UI initialization. A
        // repaint consumes that tree; it must not construct windows or invoke
        // lifecycle callbacks at the display refresh rate.
        let (width, height) = renderer.screen_size();
        game_client::gui::w3d_gadget_draw::ensure_control_bar_wnd_draw_callbacks_for_owner(
            self.windows,
        );
        // W3DInGameUI.cpp:388-391: issued-order hints precede winRepaint.
        if let Some(camera) = radar {
            for hint in &self.hints {
                let elapsed = self.client_frame.wrapping_sub(hint.creation_frame);
                if elapsed > 40 {
                    continue;
                }
                let Some(center) = camera.project_hint(hint.position) else {
                    continue;
                };
                let t = elapsed as f32 / 40.0;
                let (rgb, base) = match hint.kind {
                    game_client::gui::command_hints::CommandHintKind::Attack => {
                        ([1.0, 0.25, 0.2], 15.0)
                    }
                    _ => ([0.2, 1.0, 0.2], 19.0),
                };
                let color = [rgb[0], rgb[1], rgb[2], 0.15 + 0.75 * (1.0 - t)];
                let radius = 3.0 + base * (1.0 - 0.6 * t);
                for index in 0..16 {
                    let a = index as f32 * std::f32::consts::TAU / 16.0;
                    let b = (index + 1) as f32 * std::f32::consts::TAU / 16.0;
                    renderer.draw_line(
                        center + glam::Vec2::new(a.cos(), a.sin()) * radius,
                        center + glam::Vec2::new(b.cos(), b.sin()) * radius,
                        2.0,
                        color,
                        0.0,
                    );
                }
                renderer.draw_rect(
                    game_client::gui::ui_renderer::UIRect::new(
                        center.x - 1.5,
                        center.y - 1.5,
                        3.0,
                        3.0,
                    ),
                    color,
                    0.0,
                );
            }
        }
        self.windows
            .with_shell_ref(|shell| {
                let mut context =
                    WindowDrawContext::new(Some(&mut *renderer), self.display.clone());
                context.ime = Some(self.ime);
                context.shell_scheme = shell
                    .is_shell_active()
                    .then(|| shell.shell_menu_scheme_manager());
                self.windows.with_manager(|windows| {
                    if width > 0 && height > 0 {
                        windows.set_screen_size(width as i32, height as i32);
                    }
                    if let Some(radar) = radar {
                        radar.draw_windows(windows, &mut context, self.ui.as_deref_mut());
                    } else {
                        windows.draw_all(&mut context);
                    }
                });
            })
            .ok_or_else(|| {
                ww3d_renderer_3d::Error::InvalidOperation(
                    "native UI has no receiving Shell scheme owner".into(),
                )
            })?;
        // W3DMouse.cpp:565-567: draw the receiving mouse tooltip after windows.
        if let Some(mouse) = self.mouse.as_ref() {
            let mut mouse = mouse.lock().map_err(|_| {
                ww3d_renderer_3d::Error::InvalidOperation("native UI mouse poisoned".into())
            })?;
            if self.loading {
                mouse.set_cursor_tooltip(String::new(), None, None, None);
            }
            if !game_client::gui::ui_globals::cursor_tooltip_already_submitted_from_mouse(&mouse) {
                game_client::gui::ui_globals::tick_cursor_tooltip_from_mouse(&mut mouse);
            }
            game_client::gui::ui_globals::submit_cursor_tooltip_from_mouse(renderer, &mut mouse);
        }
        if renderer.queued_draw_command_count() > 0 {
            let color = frame.color_view_arc();
            let mut pass = frame
                .encoder()
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("receiving native UI overlay"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color.as_ref(),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            renderer
                .render(&mut pass)
                .map_err(|error| ww3d_renderer_3d::Error::GenericError(error.to_string()))?;
        }
        renderer.end_frame();
        cleanup.disarm();
        Ok(())
    }
}

trait UiFrameLifecycle {
    fn is_frame_open(&self) -> bool;
    fn end_frame(&mut self);
}
impl UiFrameLifecycle for UIRenderer {
    fn is_frame_open(&self) -> bool {
        UIRenderer::is_frame_open(self)
    }
    fn end_frame(&mut self) {
        UIRenderer::end_frame(self);
    }
}
/// Ordinary exclusive cleanup: unwind never reacquires a lock or selects a peer.
struct UiFrameCleanup<'a, T: UiFrameLifecycle> {
    renderer: &'a mut T,
    armed: bool,
}
impl<'a, T: UiFrameLifecycle> UiFrameCleanup<'a, T> {
    fn new(renderer: &'a mut T) -> Self {
        Self {
            renderer,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl<T: UiFrameLifecycle> Drop for UiFrameCleanup<'_, T> {
    fn drop(&mut self) {
        if self.armed && self.renderer.is_frame_open() {
            self.renderer.end_frame();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct TestFrame {
        open: bool,
        ends: usize,
    }
    impl UiFrameLifecycle for TestFrame {
        fn is_frame_open(&self) -> bool {
            self.open
        }
        fn end_frame(&mut self) {
            self.open = false;
            self.ends += 1;
        }
    }
    #[test]
    fn unfinished_frame_is_discarded_on_callback_unwind() {
        let mut frame = TestFrame {
            open: true,
            ends: 0,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _cleanup = UiFrameCleanup::new(&mut frame);
            panic!("WND callback unwind");
        }));
        assert!(result.is_err());
        assert!(!frame.open);
        assert_eq!(frame.ends, 1);
    }
}
