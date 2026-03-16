use anyhow::Result;
use experimental_engine_core::render_wgpu::WgpuRenderer;
use experimental_engine_core::{Engine, EngineMode};
use std::sync::Arc;
use std::time::Instant;
use winit::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

struct RuntimeState {
    engine: Engine,
    renderer: WgpuRenderer,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    last_frame_at: Instant,
    cursor_xy: (f32, f32),
}

impl RuntimeState {
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self) -> Result<()> {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_frame_at)
            .as_secs_f32()
            .min(0.05);
        self.last_frame_at = now;
        self.engine.update(dt);
        let snapshot = self.engine.snapshot();

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                return Ok(());
            }
            Err(err) => return Err(anyhow::anyhow!("surface acquire failed: {err:?}")),
        };

        let target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer
            .render_snapshot(&self.device, &self.queue, &target, &snapshot)?;
        frame.present();
        Ok(())
    }
}

async fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let window_attributes = winit::window::Window::default_attributes()
        .with_title("Experimental Game Runtime")
        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
    #[allow(deprecated)]
    let window = Arc::new(event_loop.create_window(window_attributes)?);

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let surface = instance.create_surface(window.clone())?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            label: Some("experimental-runtime-device"),
        })
        .await?;

    let size = window.inner_size();
    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .find(|format| format.is_srgb())
        .copied()
        .unwrap_or(caps.formats[0]);
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    let mut state = RuntimeState {
        engine: {
            let mut engine = Engine::new();
            engine.set_mode(EngineMode::Play);
            engine
        },
        renderer: WgpuRenderer::new(),
        surface,
        device,
        queue,
        config,
        last_frame_at: Instant::now(),
        cursor_xy: (0.0, 0.0),
    };

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => state.resize(size.width, size.height),
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                state.cursor_xy = (position.x as f32, position.y as f32);
                state
                    .engine
                    .on_viewport_mouse_move(state.cursor_xy.0, state.cursor_xy.1);
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: button_state,
                        button,
                        ..
                    },
                ..
            } if button == MouseButton::Left => match button_state {
                ElementState::Pressed => state
                    .engine
                    .on_viewport_mouse_down(state.cursor_xy.0, state.cursor_xy.1),
                ElementState::Released => state.engine.on_viewport_mouse_up(),
            },
            Event::WindowEvent {
                event: WindowEvent::MouseWheel { delta, .. },
                ..
            } => {
                let zoom_delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y as f32) * 0.01,
                };
                state.engine.on_viewport_zoom_delta(zoom_delta);
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                if let Err(err) = state.render() {
                    eprintln!("render failed: {err:?}");
                    elwt.exit();
                }
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        }
    })?;

    Ok(())
}

fn main() -> Result<()> {
    pollster::block_on(run())
}
