use gamelogic::object::draw::{W3DModelDraw, W3DTankDraw, client_visual::NativeTerrainTrackBinding};

use super::{BasicDrawable, DrawModule};

fn tank_model(module: &mut dyn DrawModule) -> Option<&mut W3DModelDraw> {
    module
        .logic_module_mut()?
        .as_any_mut()
        .downcast_mut::<W3DTankDraw>()
        .map(W3DTankDraw::receiving_model_track)
}

impl BasicDrawable {
    pub(crate) fn receiving_track_bindings(&mut self) -> Vec<NativeTerrainTrackBinding> {
        self.draw_modules
            .iter_mut()
            .filter_map(|module| tank_model(module.as_mut())?.receiving_terrain_track().cloned())
            .collect()
    }

    pub(crate) fn receiving_tank_track_model(&mut self, tag: &str) -> Option<&mut W3DModelDraw> {
        let module = self.draw_modules
            .iter_mut()
            .find(|module| module.snapshot_module_identifier() == Some(tag))?;
        tank_model(module.as_mut())
    }

    pub(crate) fn visit_receiving_tracks(
        &mut self,
        mut visit: impl FnMut(&NativeTerrainTrackBinding, bool),
    ) {
        for module in &mut self.draw_modules {
            let Some(model) = tank_model(module.as_mut()) else { continue };
            if let Some(binding) = model.receiving_terrain_track() {
                visit(binding, model.track_obscured_by_shroud());
            }
        }
    }
}
