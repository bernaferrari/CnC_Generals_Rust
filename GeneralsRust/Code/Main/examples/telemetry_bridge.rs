//! Diagnostic bridge driver for integration telemetry.
//!
//! Run with:
//! `cargo run -p generals_main --example telemetry_bridge --features integration-diagnostics`

use anyhow::Result;
use generals_main::command_line::{self, CommandLineArgs};
use generals_main::integration_bridge::IntegrationTelemetryBridge;
use integration::IntegrationConfig;
use std::sync::Arc;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let window = Arc::new(WindowBuilder::new().build(&event_loop)?);

    let cmd_args = Arc::new(
        command_line::initialize_command_line().unwrap_or_else(|_| CommandLineArgs::default()),
    );

    let mut engine =
        generals_main::cnc_game_engine::CnCGameEngine::new(window.clone(), cmd_args).await?;
    let mut bridge = IntegrationTelemetryBridge::new(IntegrationConfig::default()).await?;

    for _ in 0..120 {
        let timing = bridge.pump(&mut engine).await?;
        engine.update_with_timing(&timing);
        engine.render()?;
    }

    bridge.shutdown().await?;
    Ok(())
}
