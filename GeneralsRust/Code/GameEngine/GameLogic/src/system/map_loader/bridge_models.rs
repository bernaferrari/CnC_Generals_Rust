//! Resolve logical deck geometry from pristine model art.

use std::collections::HashMap;
use std::sync::Mutex;

use game_engine::common::system::{file::FileAccess, file_system::FileSystem};
use ww3d_core::model_geometry::rigid_section_lateral_bounds;

use super::{BridgeData, BridgeEndpoints};

/// Per-map model bounds. Spans using the same model share one asset read.
#[derive(Default)]
pub struct BridgeModels {
    bounds: HashMap<String, Option<(f32, f32)>>,
}

impl BridgeModels {
    pub fn deck(
        &mut self,
        span: BridgeEndpoints,
        model: &str,
        scale: f32,
        read_model: impl FnOnce(&str) -> Result<Vec<u8>, String>,
    ) -> Option<BridgeData> {
        let model = model.to_ascii_lowercase();
        let &(min, max) = self
            .bounds
            .entry(model)
            .or_insert_with_key(|model| match load_section_bounds(model, read_model) {
                Ok(bounds) => Some(bounds),
                Err(error) => {
                    log::warn!(
                        "Map bridge '{}' could not load model '{model}': {error}",
                        span.template_name
                    );
                    None
                }
            })
            .as_ref()?;
        Some(BridgeData::from_model_bounds(span, min, max, scale))
    }
}

fn load_section_bounds(
    model: &str,
    read_model: impl FnOnce(&str) -> Result<Vec<u8>, String>,
) -> Result<(f32, f32), String> {
    let stem = model
        .split('.')
        .next()
        .filter(|stem| !stem.is_empty())
        .ok_or("missing BridgeModelName")?;
    let path = format!("art/w3d/{stem}.w3d");
    let bytes = read_model(&path)?;
    rigid_section_lateral_bounds(&bytes, model, "BRIDGE_LEFT").map_err(|error| error.to_string())
}

/// Open the file through a receiving application's mounts.
pub fn read_mounted_bridge_model(files: &Mutex<FileSystem>, path: &str) -> Result<Vec<u8>, String> {
    let mut file = files
        .lock()
        .map_err(|error| error.to_string())?
        .open_mounted_file(path, FileAccess::READ.combine(FileAccess::BINARY))
        .ok_or_else(|| format!("missing {path}"))?;
    file.read_entire_and_close()
        .map_err(|error| error.to_string())
}
