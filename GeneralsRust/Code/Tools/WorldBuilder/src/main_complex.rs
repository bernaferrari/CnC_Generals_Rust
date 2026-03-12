//! World Builder - Advanced Level Editor for Command & Conquer
//! 
//! A modern, powerful level editor with real-time 3D editing, terrain sculpting,
//! object placement, scripting, and advanced lighting systems.

mod editor;
mod terrain;
mod objects;
mod scripting;
mod map;
mod tools;
mod ui;

use anyhow::Result;
use editor::WorldBuilderTool;
use ui_framework::{ToolApp, GameTool};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    log::info!("Starting Command & Conquer World Builder v{}", env!("CARGO_PKG_VERSION"));
    
    // Create the World Builder tool
    let world_builder = Box::new(WorldBuilderTool::new()?);
    
    // Create and run the application
    let app = ToolApp::new(world_builder)?;
    app.run()?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_world_builder_creation() {
        let world_builder = WorldBuilderTool::new();
        assert!(world_builder.is_ok());
    }
}