#[cfg(not(feature = "dev-tools"))]
fn main() {
    eprintln!("Enable the 'dev-tools' feature to build and run wgpu_ui_demo.");
}

#[cfg(feature = "dev-tools")]
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(feature = "dev-tools")]
fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("WGPU UI Demo (Smoke Test)")
        .with_inner_size(winit::dpi::LogicalSize::new(960.0, 540.0))
        .build(&event_loop)?;

    println!("wgpu_ui_demo window initialized: {:?}", window.inner_size());

    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Wait);
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => target.exit(),
            Event::AboutToWait => window.request_redraw(),
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Smoke test entrypoint: rendering integration lives in the main game binary.
            }
            _ => {}
        }
    })?;

    Ok(())
}
