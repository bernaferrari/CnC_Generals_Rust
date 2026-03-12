use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 MINIMAL WINDOW TEST");

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Minimal Test Window")
        .with_inner_size(winit::dpi::LogicalSize::new(400, 300))
        .build(&event_loop)?;

    println!("✅ Window created successfully");

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("Window close requested");
                elwt.exit();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                println!("⚠️ Redraw requested - but no graphics context!");
            }
            _ => {}
        }
    })?;

    Ok(())
}
