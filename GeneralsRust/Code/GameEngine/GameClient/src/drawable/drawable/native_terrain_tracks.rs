use gamelogic::object::{
    draw::{W3DModelDraw, client_visual::NativeTerrainTrackBinding},
    drawable::with_native_terrain_track_model,
};

use super::BasicDrawable;

impl BasicDrawable {
    pub(crate) fn receiving_track_bindings(&mut self) -> Vec<NativeTerrainTrackBinding> {
        self.draw_modules
            .iter_mut()
            .filter_map(|slot| {
                with_native_terrain_track_model(slot.logic_module_mut()?, |model| {
                    model.receiving_terrain_track().cloned()
                })
                .flatten()
            })
            .collect()
    }

    pub(crate) fn with_receiving_track_model<R>(
        &mut self,
        tag: &str,
        visit: impl FnOnce(&mut W3DModelDraw) -> R,
    ) -> Option<R> {
        let module = self
            .draw_modules
            .iter_mut()
            .find(|module| module.snapshot_module_identifier() == Some(tag))?
            .logic_module_mut()?;
        with_native_terrain_track_model(module, visit)
    }

    pub(crate) fn visit_receiving_tracks(
        &mut self,
        mut visit: impl FnMut(&NativeTerrainTrackBinding, bool),
    ) {
        for slot in &mut self.draw_modules {
            if let Some(module) = slot.logic_module_mut() {
                let _ = with_native_terrain_track_model(module, |model| {
                    if let Some(binding) = model.receiving_terrain_track() {
                        visit(binding, model.track_obscured_by_shroud());
                    }
                });
            }
        }
    }
}
