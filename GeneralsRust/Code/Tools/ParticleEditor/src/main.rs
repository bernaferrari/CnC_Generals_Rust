//! Particle Editor - Advanced Real-Time Particle System Editor
//!
//! A modern particle system editor with real-time preview, timeline-based editing,
//! GPU acceleration, and advanced physics simulation for Command & Conquer.

mod editor;
mod export;
mod particles;
mod preview;
mod timeline;
mod ui;

use anyhow::Result;
use editor::ParticleEditorTool;
use ui_framework::{GameTool, ToolApp};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!(
        "Starting Command & Conquer Particle Editor v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Create the Particle Editor tool
    let particle_editor = Box::new(ParticleEditorTool::new()?);

    // Create and run the application
    let app = ToolApp::new(particle_editor)?;
    app.run()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_editor_creation() {
        let particle_editor = ParticleEditorTool::new();
        assert!(particle_editor.is_ok());
    }
}
