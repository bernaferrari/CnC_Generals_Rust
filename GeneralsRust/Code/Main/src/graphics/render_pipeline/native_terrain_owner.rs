//! Explicit native map presentation receiver; construction never publishes it.
use super::*;

/// Convert one admitted prop's authored model into render items. The caller
/// resolves the model and culls its bounds before calling this function.
#[cfg(feature = "game_client")]
pub fn append_terrain_prop_model_items(
    prop: &game_client::terrain::terrain_visual::TerrainPropRecord,
    model: &W3DModel,
    shroud: gamelogic::common::types::ObjectShroudStatus,
    camera_position: Vec3,
    output: &mut Vec<RenderItem>,
) {
    use gamelogic::common::types::ObjectShroudStatus;
    if matches!(
        shroud,
        ObjectShroudStatus::Shrouded | ObjectShroudStatus::Invalid
    ) {
        return;
    }
    let world = prop.render_world_transform();
    let position = world.w_axis.truncate();
    for (mesh_index, mesh) in model.meshes.iter().enumerate() {
        let Some((local, visible)) =
            model.mesh_local_transform_and_visibility_for_binding(mesh_index, None, 0.0)
        else {
            continue;
        };
        if !visible {
            continue;
        }
        let pass = RenderPipeline::render_pass_for_material(&mesh.material);
        let mut item = RenderItem::new_unbound_client_drawable(
            prop.drawable_id,
            model.name.clone(),
            mesh_index,
            position,
            world,
            &mesh.material,
            pass,
        );
        item.owner = RenderItemOwner::TerrainProp(prop.drawable_id);
        item.distance = position.distance(camera_position);
        item.set_mesh_local_transform(local);
        item.terrain_prop_projected_shroud = shroud != ObjectShroudStatus::Clear;
        super::pipeline_collect::stamp_skinned_hierarchy_bind_pose(&mut item, mesh);
        output.push(item);
    }
}
impl RenderPipeline {
    #[cfg(feature = "game_client")]
    pub(crate) fn bind_native_prop_epoch(&mut self, epoch: u64) {
        self.native_draw_world
            .pending_native_props
            .bind_epoch(epoch);
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn receive_native_prop(
        &mut self,
        host_epoch: u64,
        request: gamelogic::object::draw::w3d_prop_draw::TerrainPropAddRequest,
    ) {
        if !self
            .native_draw_world
            .pending_native_props
            .push(host_epoch, request)
        {
            warn!("rejected terrain prop from a different draw-world epoch");
            return;
        }
        self.flush_native_props();
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn flush_native_props(&mut self) {
        let Some(terrain) = self.native_terrain.as_mut() else {
            return;
        };
        self.native_draw_world
            .pending_native_props
            .drain_into(terrain);
    }

    /// Submit the render objects retained by the receiving terrain's prop
    /// buffer. W3D source vertices and child transforms are already Y-up.
    #[cfg(feature = "game_client")]
    pub(crate) fn append_native_terrain_prop_models(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        allow_sync_model_loads: bool,
        deferred_model_load_budget: &mut usize,
    ) {
        use gamelogic::common::types::ObjectShroudStatus;
        let Some(terrain) = self.native_terrain.as_ref() else {
            return;
        };
        let planes = extract_frustum_planes(&(*projection_matrix * *view_matrix));
        // Many props share one authored type. Resolve each spelling once per
        // frame, including deferred and missing models, before instance cull.
        let mut models: HashMap<&str, Option<Arc<W3DModel>>> = HashMap::new();
        for prop in terrain.terrain_props() {
            let shroud =
                self.presentation_frame
                    .as_ref()
                    .map_or(ObjectShroudStatus::Clear, |frame| {
                        if frame.fow_shell_bypass {
                            ObjectShroudStatus::Clear
                        } else {
                            frame
                                .fow_grid
                                .prop_shroud_status(prop.position[0], prop.position[1])
                        }
                    });
            if matches!(
                shroud,
                ObjectShroudStatus::Shrouded | ObjectShroudStatus::Invalid
            ) {
                continue;
            }
            let model = models.entry(&prop.model_name).or_insert_with(|| {
                match Self::ensure_render_model_loaded(
                    self.forward_pass.application_assets.as_deref(),
                    &mut self.failed_model_resolutions,
                    graphics_system,
                    &prop.model_name,
                    &prop.model_name,
                    allow_sync_model_loads,
                    deferred_model_load_budget,
                ) {
                    // W3DPropBuffer rejects a missing prop type even when the
                    // general resolver's debug-cube option is enabled.
                    RenderModelLoadResult::Ready(model)
                        if model.name
                            != crate::assets::mesh_asset_resolve::PLACEHOLDER_MODEL_KEY =>
                    {
                        Some(model)
                    }
                    _ => None,
                }
            });
            let Some(model) = model.as_ref() else {
                continue;
            };
            let world = prop.render_world_transform();
            let center =
                world.transform_point3((model.bounding_box_min + model.bounding_box_max) * 0.5);
            let radius =
                (model.bounding_box_max - model.bounding_box_min).length() * 0.5 * prop.scale.abs();
            if !world_sphere_in_expanded_frustum(&planes, center, radius, camera_position) {
                continue;
            }
            append_terrain_prop_model_items(
                prop,
                &model,
                shroud,
                camera_position,
                &mut self.render_items,
            );
        }
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn borrow_native_installed_poses_and_terrain(
        &mut self,
    ) -> (
        native_pose_loan::NativeInstalledPoseLoan<'_>,
        Option<&mut game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) {
        let poses = self
            .native_draw_world
            .borrow_installed_poses(&mut self.failed_model_resolutions, self.animation_sync_ms, self.forward_pass.application_assets.clone());
        (poses, self.native_terrain.as_mut())
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn native_terrain(
        &self,
    ) -> Option<&game_client::terrain::terrain_visual::TerrainVisualImpl> {
        self.native_terrain.as_ref()
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn native_terrain_mut(
        &mut self,
    ) -> Option<&mut game_client::terrain::terrain_visual::TerrainVisualImpl> {
        self.native_terrain.as_mut()
    }
    /// C++ `W3DGameClient::adjustLOD` for this pipeline: the texture
    /// reduction goes to its own renderer runtime and installed terrain.
    #[cfg(feature = "game_client")]
    pub(crate) fn apply_texture_lod(&mut self, target_factor: i32) {
        let r3d = std::sync::Arc::clone(&self.forward_pass.renderer.renderer_3d);
        let _ = game_client::core::game_client::apply_lod_texture_reduction_to_session(
            &r3d,
            target_factor,
            &mut |reduction| {
                if let Some(terrain) = self.native_terrain.as_mut() {
                    terrain.apply_texture_lod_reduction(reduction);
                }
            },
        );
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn clear_native_terrain(&mut self) {
        self.native_terrain = None;
        self.native_draw_world.pending_native_props.clear();
        self.radar_terrain_colors = None;
        self.radar_terrain_colors_generation =
            self.radar_terrain_colors_generation.wrapping_add(1);
        self.heightmap_world_size = None;
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_terrain_owner_tests.rs"]
mod tests;
