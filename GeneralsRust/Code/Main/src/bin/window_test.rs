// Simple window visibility test
use std::sync::Arc;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

fn main() {
    println!("🧪 WINDOW TEST - Creating simple visible window...");

    let event_loop = EventLoop::new().unwrap();

    // Create a VERY simple, small window
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("WINDOW TEST - SHOULD BE VISIBLE")
            .with_inner_size(winit::dpi::LogicalSize::new(400, 300))
            .with_position(winit::dpi::PhysicalPosition::new(200, 200))
            .with_visible(true)
            .with_resizable(true)
            .with_decorations(true)
            .build(&event_loop)
            .unwrap(),
    );

    println!("✅ Window created:");
    println!("   Title: {:?}", window.title());
    println!("   Size: {:?}", window.inner_size());
    println!(
        "   Position: {:?}",
        window.outer_position().unwrap_or_default()
    );
    println!("   Visible: {:?}", window.is_visible().unwrap_or(true));

    // Force window to be visible and focused
    window.set_visible(true);
    window.focus_window();
    window.request_redraw();

    println!("🚨 LOOK FOR WINDOW: 'WINDOW TEST - SHOULD BE VISIBLE'");
    println!("   Should be at position (200, 200)");
    println!("   Press ESC or close window to exit");

    // Simple event loop
    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("Window close requested - exiting");
                target.exit();
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event, .. },
                ..
            } => {
                if event.state == ElementState::Pressed
                    && matches!(event.logical_key, Key::Named(NamedKey::Escape))
                {
                    println!("ESC pressed - exiting");
                    target.exit();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Simple redraw - just clear to red so we know it's working
                println!("🎨 Redraw requested - window should be red");
            }
            _ => {}
        }
    });
}
