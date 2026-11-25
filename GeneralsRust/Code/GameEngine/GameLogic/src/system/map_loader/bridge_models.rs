//! Resolve logical deck geometry from a match's pristine bridge art.

use std::collections::HashMap;

use game_engine::common::common_services::CommonServices;
use game_engine::common::system::file::FileAccess;
use ww3d_core::model_geometry::rigid_section_lateral_bounds;

use super::{BridgeData, BridgeEndpoints};

pub(super) struct BridgeModels<'a> {
    common: &'a CommonServices,
    bounds: HashMap<String, Option<(f32, f32)>>,
}

impl<'a> BridgeModels<'a> {
    pub(super) fn new(common: &'a CommonServices) -> Self {
        Self {
            common,
            bounds: HashMap::new(),
        }
    }

    pub(super) fn deck(&mut self, span: BridgeEndpoints) -> Option<BridgeData> {
        let Some((model, scale)) = self
            .common
            .ini
            .terrain_roads
            .read()
            .find_bridge(&span.template_name.as_str().into())
            .map(|bridge| (bridge.model_name.to_ascii_lowercase(), bridge.bridge_scale))
        else {
            log::warn!(
                "Map bridge '{}' has no Bridge definition",
                span.template_name
            );
            return None;
        };
        let &(min, max) = self
            .bounds
            .entry(model.clone())
            .or_insert_with(|| match load_section_bounds(self.common, &model) {
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

fn load_section_bounds(common: &CommonServices, model: &str) -> Result<(f32, f32), String> {
    let stem = model
        .split('.')
        .next()
        .filter(|stem| !stem.is_empty())
        .ok_or("missing BridgeModelName")?;
    let path = format!("art/w3d/{stem}.w3d");
    let mut file = common
        .application
        .file_system
        .lock()
        .map_err(|error| error.to_string())?
        .open_mounted_file(&path, FileAccess::READ.combine(FileAccess::BINARY))
        .ok_or_else(|| format!("missing {path}"))?;
    let bytes = file
        .read_entire_and_close()
        .map_err(|error| error.to_string())?;
    rigid_section_lateral_bounds(&bytes, model, "BRIDGE_LEFT").map_err(|error| error.to_string())
}
