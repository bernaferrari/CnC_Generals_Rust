//! Receiving resident Drawable definitions, independent of mesh visibility.
use super::*;
impl PresentationFrame {
    pub(crate) fn native_resident_drawable_inputs(&self) -> Vec<UnitRenderInput> {
        self.direct_host_drawables
            .iter()
            .filter(|direct| direct.resident)
            .map(|direct| {
                let mut input = UnitRenderInput::from_renderable_with_environment(
                    &direct.object,
                    self.world_env.is_snow,
                    self.world_env.is_night,
                    self.frame.0,
                );
                input.structure_topple_matrix =
                    self.structure_topple_transforms.get(&input.id).copied();
                if !direct.visual_template_name.trim().is_empty() {
                    input.template_name = direct.visual_template_name.clone();
                    input.receiving_draw_models = direct.receiving_visual_draw_models.clone();
                    if direct.visual_mesh_scale.is_finite() {
                        input.mesh_scale = direct.visual_mesh_scale;
                    }
                }
                input.apply_debris_model_override(direct.debris_model_override.as_deref());
                input
            })
            .collect()
    }
}
