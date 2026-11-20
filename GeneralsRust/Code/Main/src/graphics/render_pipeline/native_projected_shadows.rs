//! W3DModelDraw Shadow = SHADOW_DECAL on the receiving terrain and draw roster.
use super::*;
use crate::game_logic::host_enum_table_residual::SHADOW_DECAL;
use crate::graphics::render_item::RenderItemOwner;
use game_client::terrain::terrain_visual::ProjectedUnitShadow;
use ww3d_renderer_3d::rendering::wgpu_main_renderer::{FramePhase, ScopedNativePass};

impl RenderPipeline {
    pub(crate) fn admit_native_projected_shadows(
        &mut self,
        world: &crate::game_logic::GameLogic,
        read_texture: impl FnMut(&str) -> Option<Vec<u8>>,
    ) -> HashSet<u32> {
        let Some(terrain) = self.native_terrain.as_mut() else {
            return HashSet::new();
        };
        let mut shadows = Vec::new();
        if game_engine::common::game_lod::use_shadow_decals()
            && let Some(frame) = self.presentation_frame.as_ref()
        {
            for unit in &frame.objects {
                if unit.shadow_type_bits != SHADOW_DECAL
                    || unit.destroyed
                    || !unit.shadows_enabled
                    || !crate::graphics::selection_renderer::shadow_visible_through_shroud(
                        unit.drawable_shroud,
                        unit.fow_visibility,
                    )
                {
                    continue;
                }
                let Some(object) = world.host_object(unit.id) else {
                    continue;
                };
                if unit
                    .draw_playback_identity
                    .as_ref()
                    .is_none_or(|identity| !world.matches_live_draw_playback_identity(object, identity))
                {
                    continue;
                }
                let template = &object.thing().template;
                let Some(texture_name) = template
                    .shadow_texture
                    .as_deref()
                    .filter(|name| !name.is_empty())
                else {
                    continue;
                };
                let size = glam::Vec2::new(template.shadow_size_x, template.shadow_size_y);
                let offset = glam::Vec2::new(template.shadow_offset_x, template.shadow_offset_y);
                if !size.is_finite() || size.min_element() <= 0.0 || !offset.is_finite() {
                    continue;
                }
                let layer =
                    gamelogic::path::PathfindLayerEnum::from_u32(object.pathfind_layer.into());
                let layer_height = if layer == gamelogic::path::PathfindLayerEnum::Ground {
                    None
                } else {
                    let Ok(terrain) = world.world_services.terrain().read() else {
                        continue;
                    };
                    Some(terrain.get_layer_height(
                        unit.position.x,
                        unit.position.z,
                        layer,
                        None,
                        true,
                    ))
                };
                shadows.push(ProjectedUnitShadow {
                    object_id: unit.id.0,
                    texture_name: texture_name.to_owned(),
                    size,
                    offset,
                    transform: Mat4::from_translation(unit.position),
                    layer_height,
                });
            }
        }
        let admitted = terrain.admit_projected_unit_shadows(shadows, read_texture);
        if !admitted.is_empty() {
            self.forward_pass.renderer.enqueue_scoped_native_pass(
                FramePhase::DecalShadows,
                ScopedNativePass::ProjectedDecalShadows,
            );
        }
        admitted
    }

    pub(super) fn synchronize_native_projected_shadow_casters(&mut self) {
        let Some(terrain) = self
            .native_terrain
            .as_mut()
            .filter(|terrain| terrain.has_projected_unit_shadow_inputs())
        else {
            return;
        };
        // W3DProjectedShadow only queues a model that is really visible.
        // The already-culled scene items carry the receiving Draw visit's
        // model matrix, separate from its per-mesh/bone transform.
        let mut visible = game_engine::common::fast_hash::FxHashMap::default();
        for item in &self.render_items {
            if let RenderItemOwner::Object(id) = item.owner {
                visible.entry(id.0).or_insert(item.world_matrix);
            }
        }
        terrain.retain_projected_unit_shadow_casters(|id| visible.get(&id).copied());
    }
}
