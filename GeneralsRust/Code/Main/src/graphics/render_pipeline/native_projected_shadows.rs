//! W3DModelDraw Shadow = SHADOW_DECAL on the receiving terrain and draw roster.
use super::*;
use crate::game_logic::{host_enum_table_residual::SHADOW_DECAL, object::Object};
use crate::graphics::render_item::RenderItemOwner;
use crate::presentation_frame::RenderableObject;
use game_client::terrain::terrain_visual::ProjectedUnitShadow;
use game_engine::common::fast_hash::{FxHashMap, FxHashSet};
use glam::Vec2;
use ww3d_renderer_3d::rendering::wgpu_main_renderer::{FramePhase, ScopedNativePass};

fn live_shadow_object<'a>(
    world: &'a crate::game_logic::GameLogic,
    unit: &RenderableObject,
) -> Option<&'a Object> {
    if unit.destroyed
        || !unit.shadows_enabled
        || !crate::graphics::selection_renderer::shadow_visible_through_shroud(
            unit.drawable_shroud,
            unit.fow_visibility,
        )
    {
        return None;
    }
    let object = world.host_object(unit.id)?;
    let identity = unit.draw_playback_identity.as_ref()?;
    world
        .matches_live_draw_playback_identity(object, identity)
        .then_some(object)
}

fn shadow_layer_height(
    world: &crate::game_logic::GameLogic,
    object: &Object,
    unit: &RenderableObject,
) -> Option<Option<f32>> {
    use generals_sim::pathfinding::PathfindLayerEnum;

    let layer = PathfindLayerEnum::from_u32(object.pathfind_layer.into());
    if layer == PathfindLayerEnum::Ground {
        return Some(None);
    }
    let terrain = world.world_services.terrain().read().ok()?;
    Some(Some(terrain.get_layer_height(
        unit.position.x,
        unit.position.z,
        layer,
        None,
        true,
    )))
}

fn projected_size(authored: Vec2, bounds: Option<Vec2>) -> Option<Vec2> {
    let mut size = authored;
    if size.x == 0.0 {
        size.x = bounds?.x;
    }
    if size.y == 0.0 {
        size.y = bounds?.y;
    }
    (size.is_finite() && size.min_element() > 0.0).then_some(size)
}

impl RenderPipeline {
    pub(crate) fn admit_native_projected_shadows(
        &mut self,
        world: &crate::game_logic::GameLogic,
        use_shadow_decals: bool,
        read_texture: impl FnMut(&str) -> Option<Vec<u8>>,
    ) -> HashSet<u32> {
        let Some(terrain) = self.native_terrain.as_mut() else {
            return HashSet::new();
        };
        let mut shadows = Vec::new();
        let mut handled = HashSet::new();
        if use_shadow_decals && let Some(frame) = self.presentation_frame.as_ref()
        {
            for direct in &frame.direct_host_drawables {
                let unit = &direct.object;
                // A tree Draw queues its own shared decal from the terrain
                // roster. A missing decal asset cannot become a model disc.
                if direct.receiving_client_template.as_ref()
                    .is_some_and(|template| template.has_only_tree_draws())
                {
                    handled.insert(unit.id.0);
                    continue;
                }
                let Some((texture_name, size, offset)) = direct
                    .receiving_client_template
                    .as_ref()
                    .and_then(|template| template.projected_model_decal())
                else {
                    continue;
                };
                let Some(source) = direct.receiving_visual_draw_models.as_ref() else {
                    continue;
                };
                let mut modules = source.model_draw_shadow_module_indices().peekable();
                if modules.peek().is_none() || !direct.resident {
                    continue;
                }
                let Some(object) = live_shadow_object(world, unit) else {
                    continue;
                };
                // This source path owns DECAL even without a render object or
                // resident texture. C++ does not substitute a round blob.
                handled.insert(unit.id.0);
                if !size.is_finite() || size.min_element() < 0.0 || !offset.is_finite() {
                    continue;
                }
                let Some(layer_height) = shadow_layer_height(world, object, unit) else {
                    continue;
                };
                for module_index in modules {
                    shadows.push(ProjectedUnitShadow {
                        object_id: unit.id.0,
                        module_index: Some(module_index),
                        texture_name: texture_name.to_owned(),
                        size,
                        offset,
                        transform: Mat4::from_translation(unit.position),
                        layer_height,
                    });
                }
            }
            for unit in frame.drawable_objects() {
                if unit.shadow_type_bits != SHADOW_DECAL || handled.contains(&unit.id.0) {
                    continue;
                }
                let Some(object) = live_shadow_object(world, unit) else {
                    continue;
                };
                let template = &object.thing().template;
                let Some(texture_name) = template
                    .shadow_texture
                    .as_deref()
                    .filter(|name| !name.is_empty())
                else {
                    continue;
                };
                let size = Vec2::new(template.shadow_size_x, template.shadow_size_y);
                let offset = Vec2::new(template.shadow_offset_x, template.shadow_offset_y);
                if !size.is_finite() || size.min_element() <= 0.0 || !offset.is_finite() {
                    continue;
                }
                let Some(layer_height) = shadow_layer_height(world, object, unit) else {
                    continue;
                };
                shadows.push(ProjectedUnitShadow {
                    object_id: unit.id.0,
                    module_index: None,
                    texture_name: texture_name.to_owned(),
                    size,
                    offset,
                    transform: Mat4::from_translation(unit.position),
                    layer_height,
                });
            }
        }
        let resident = terrain.admit_projected_unit_shadows(shadows, read_texture);
        if !resident.is_empty() {
            self.forward_pass.renderer.enqueue_scoped_native_pass(
                FramePhase::DecalShadows,
                ScopedNativePass::ProjectedDecalShadows,
            );
        }
        handled.extend(resident);
        handled
    }

    pub(super) fn synchronize_native_projected_shadow_casters(&mut self) {
        let Some(terrain) = self
            .native_terrain
            .as_mut()
            .filter(|terrain| terrain.has_projected_unit_shadow_inputs())
        else {
            return;
        };
        // Wait for the final receiver-side direct scene decisions. A source
        // model removed there cannot cast through a sibling module, debug
        // fallback, or later RenderBridge/ghost item with the same actor ID.
        let mut visible = FxHashMap::default();
        let mut visible_modules = FxHashSet::default();
        for item in &self.render_items {
            if let RenderItemOwner::Object(id) = item.owner {
                visible.entry(id.0).or_insert(item.world_matrix);
                if let Some(module_index) = item.native_projected_shadow_module_index {
                    visible_modules.insert((id.0, module_index));
                }
            }
        }
        let poses = &mut self.native_draw_world.drawable_visual_states;
        terrain.resolve_projected_unit_shadow_casters(|shadow| {
            let Some(module_index) = shadow.module_index else {
                return Some((*visible.get(&shadow.object_id)?, shadow.size));
            };
            if !visible_modules.contains(&(shadow.object_id, module_index)) {
                return None;
            }
            let state = poses.get_mut(&(NativeDrawOwnerId::Actor(shadow.object_id), module_index))?;
            let installed_draw_pose::InstalledDrawObject::Ready(pose) = &mut state.installed_draw_pose
            else {
                return None;
            };
            let bounds = (shadow.size.x == 0.0 || shadow.size.y == 0.0)
                .then(|| pose.constructor_projected_decal_size())
                .flatten();
            Some((pose.world, projected_size(shadow.size, bounds)?))
        });
    }
}
