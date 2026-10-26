//! Null-Object Drawables use the same installed model/animation owner as actors.
//! Their typed client identity never enters physics, selection or Object FOW.
use super::installed_draw_pose::InstalledDrawObject;
use super::*;

impl RenderPipeline {
    pub(super) fn collect_native_auxiliary_items(
        &mut self,
        graphics: &mut GraphicsSystem,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
        observation: &game_client::core::game_client::NativeAuxiliaryDrawObservation,
        view: &Mat4,
        projection: &Mat4,
        camera: Vec3,
        _delta_time: f32,
    ) -> Result<()> {
        if observation.hidden {
            return Ok(());
        }
        let owner = NativeDrawOwnerId::Aux(receipt.drawable_id());
        let lifetime = self
            .native_draw_world
            .native_drawable_lifetimes
            .get(&owner)
            .ok_or_else(|| {
                anyhow::anyhow!("native auxiliary has no actual RenderObj constructor")
            })?;
        if !lifetime.identity.matches_auxiliary_receipt(receipt) {
            return Err(anyhow::anyhow!("stale auxiliary RenderObj continuation"));
        }
        let definitions = lifetime.auxiliary.clone().ok_or_else(|| {
            anyhow::anyhow!("auxiliary RenderObj has no receiving Draw definition")
        })?;
        let selected = definitions
            .native_render_draws(observation.condition_bits)
            .map_err(|_| anyhow::anyhow!("invalid receiving auxiliary Draw condition"))?;
        // There is no Object transform/physics branch. The actual CPU Drawable
        // supplies its independent base and instance matrices at this Draw.
        let world = Mat4::from_cols_array_2d(&observation.base.elements).transpose()
            * Mat4::from_cols_array_2d(&observation.instance.elements).transpose()
            * Mat4::from_scale(Vec3::splat(observation.scale));
        let sync = self.animation_sync_ms;
        let mut records = Vec::new();
        {
            let mut poses = self.borrow_native_installed_poses();
            for model in selected {
                let index = model.module_index;
                let state = poses
                    .drawable_visual_states
                    .get_mut(&(owner, index))
                    .ok_or_else(|| {
                        anyhow::anyhow!("auxiliary Draw module missing from constructor roster")
                    })?;
                state.mark_native_condition(observation.condition_bits, model);
                poses.install_native_condition_for_owner(owner, index, world, sync, graphics);
                let state = poses
                    .drawable_visual_states
                    .get_mut(&(owner, index))
                    .unwrap();
                let pose = match &mut state.installed_draw_pose {
                    InstalledDrawObject::NoRenderObject => continue,
                    InstalledDrawObject::AwaitingAdmission => {
                        return Err(anyhow::anyhow!(
                            "auxiliary selected Draw state lacks native RenderObj support"
                        ));
                    }
                    InstalledDrawObject::Ready(pose) => pose,
                };
                pose.install_transform(world);
                // Drawable.cpp638: a Drawable without an Object should animate.
                state.install_animation_pause(false, sync);
                records.push((index, state.installed_draw_pose.clone()));
            }
        }
        let planes = extract_frustum_planes(&(*projection * *view));
        let shroud = Self::frozen_objectless_shroud_for_owner(
            self.presentation_frame.as_deref(),
            receipt.drawable_id().0,
            Some(observation.shroud_object),
        );
        for (index, installed) in records {
            let InstalledDrawObject::Ready(pose) = installed else {
                continue;
            };
            let position = world.w_axis.truncate();
            let center = world.transform_point3(
                (pose.model.bounding_box_min + pose.model.bounding_box_max) * 0.5,
            );
            let radius = (pose.model.bounding_box_max - pose.model.bounding_box_min).length()
                * 0.5
                * observation.scale.abs();
            if !center.is_finite() || !radius.is_finite() || radius < 0.0 {
                return Err(anyhow::anyhow!(
                    "auxiliary RenderObj has invalid source bounds"
                ));
            }
            if !world_sphere_in_expanded_frustum(&planes, center, radius, camera) {
                continue;
            }
            let state = self
                .native_draw_world
                .drawable_visual_states
                .get_mut(&(owner, index))
                .unwrap();
            let sampled = state
                .admit_parent_render_hierarchy(sync)
                .map_err(|error| anyhow::anyhow!("auxiliary render hierarchy: {error:?}"))?;
            let InstalledDrawObject::Ready(pose) = &state.installed_draw_pose else {
                unreachable!()
            };
            for (mesh_index, mesh) in pose.model.meshes.iter().enumerate() {
                let Some((local, visible)) =
                    pose.model.mesh_local_transform_and_visibility_for_binding(
                        mesh_index,
                        pose.animation_binding.as_ref(),
                        sampled,
                    )
                else {
                    return Err(anyhow::anyhow!(
                        "auxiliary render child topology unavailable"
                    ));
                };
                if !visible {
                    continue;
                }
                let pass = if mesh.material.opacity * observation.opacity < 1.0 {
                    RenderPass::ForwardTransparent
                } else {
                    RenderPass::ForwardOpaque
                };
                let mut item = RenderItem::new_unbound_client_drawable(
                    receipt.drawable_id().0,
                    pose.model.name.clone(),
                    mesh_index,
                    position,
                    world,
                    &mesh.material,
                    pass,
                );
                item.distance = position.distance(camera);
                item.presentation_opacity = observation.opacity;
                item.set_mesh_local_transform(local);
                item.animation_binding = pose.animation_binding.clone();
                item.animation_frame = sampled;
                item.capture_bone_controls = pose.controls.clone();
                if let Some(shroud) = shroud {
                    item.set_frozen_objectless_drawable_shroud(shroud);
                }
                super::pipeline_collect::stamp_skinned_hierarchy_bind_pose(&mut item, mesh);
                self.render_items.push(item);
            }
        }
        Ok(())
    }
}
