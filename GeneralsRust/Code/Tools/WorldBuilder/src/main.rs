#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::single_match)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::explicit_auto_deref)]
#![allow(clippy::useless_vec)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::new_without_default)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::manual_map)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::unnecessary_unwrap)]
#![allow(clippy::for_kv_map)]
#![allow(clippy::single_char_add_str)]
#![allow(clippy::useless_format)]
#![allow(deprecated)]
#![allow(clippy::assign_op_pattern)]
//! World Builder - Advanced Level Editor for Command & Conquer
//!
//! A modern, powerful level editor with real-time 3D editing, terrain sculpting,
//! object placement, scripting, and advanced lighting systems.

mod editor;
mod map;
mod objects;
mod scripting;
mod terrain;
mod tools;
mod ui;

use anyhow::Result;
use editor::WorldBuilderTool;
use ui_framework::{GameTool, ToolApp};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!(
        "Starting Command & Conquer World Builder v{}",
        env!("CARGO_PKG_VERSION")
    );

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
    use world_builder::chrome::EditorChrome;

    #[test]
    fn test_world_builder_creation() {
        let world_builder = WorldBuilderTool::new();
        assert!(world_builder.is_ok());
    }

    #[test]
    fn chrome_menu_contains_file_open() {
        let chrome = EditorChrome::new();
        assert!(chrome.menu_contains("File", "Open"));
        let tool = WorldBuilderTool::new().expect("tool");
        assert!(tool.chrome().menu_contains("File", "Open"));
    }

    #[test]
    fn selecting_scorch_tool_sets_current_tool() {
        let mut tool = WorldBuilderTool::new().expect("tool");
        assert!(tool.set_current_tool("scorch"));
        assert_eq!(tool.current_tool_id(), "scorch");
    }
}
