#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

impl RenderPipeline {
    /// Retain the exact source stamp and authored weapon-bone bases on every
    /// RenderItem emitted from one GameClient DrawSubmission.  This is a pure
    /// transport boundary: the active bridge still does not dispatch recoil
    /// from these fields, and it never interprets a bare WeaponDischarged
    /// event.  A later renderer path must validate the names against this
    /// freshly loaded W3D hierarchy before producing any bone controls.
    #[cfg(feature = "game_client")]
    fn attach_bridge_draw_metadata(
        item: &mut RenderItem,
        submission: &game_client::render_bridge::DrawSubmission,
    ) {
        item.legacy_model_draw_source = submission.legacy_model_draw_source.clone();
        item.legacy_weapon_bone_bindings = submission.legacy_weapon_bone_bindings.clone();
        // C++ Drawable::setDrawableOpacity / colorTint travel on the submission.
        // Placement ghosts are client drawables, not FOW — keep visibility_alpha at 1.
        let opacity = submission.render_state.opacity;
        if opacity.is_finite() && (opacity - 1.0).abs() > f32::EPSILON {
            item.set_presentation_opacity(opacity);
            item.material.opacity = (item.material.opacity * opacity).clamp(0.0, 1.0);
            item.set_fow_visibility(crate::fow_rendering::ObjectVisibility {
                visibility_alpha: 1.0,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            });
        }
        if let Some(tint) = submission.render_state.construction_tint {
            let tint = glam::Vec3::new(tint[0], tint[1], tint[2]);
            item.material.diffuse_color *= tint;
            item.material.emissive_color += tint * 0.15;
        }
    }

    /// Translate the frozen GameClient W3DModelDraw animation record into the
    /// exact W3D frame used by the bridge collector. `ModelDrawState` carries
    /// a normalized 0..=1 progress fraction, so it is the authority here; do
    /// not infer a clip from RenderConditionFlags or a mesh basename.
    ///
    /// An absent animation is a deliberate bind-pose request. A named source
    /// animation which is not present in this exact W3D asset stays bind-pose
    /// too: choosing animation zero would invent visible bone state.
    fn bridge_draw_animation(
        model: &crate::assets::W3DModel,
        animation_name: Option<&str>,
        animation_time: f32,
    ) -> (Option<crate::assets::W3dAnimationBinding>, f32) {
        let Some(animation_name) = animation_name else {
            return (None, 0.0);
        };
        let Some(animation_binding) = Self::cached_draw_animation_binding(model, animation_name)
        else {
            return (None, 0.0);
        };
        let Some((num_frames, _)) = model.animation_binding_metadata(&animation_binding) else {
            return (None, 0.0);
        };
        if !animation_time.is_finite() {
            return (None, 0.0);
        }
        let frame = animation_time.clamp(0.0, 1.0) * num_frames.saturating_sub(1) as f32;
        (Some(animation_binding), frame)
    }

    /// Collect render items from game objects - equivalent to C++ RenderPipeline::CollectRenderItems()
    /// Integrates FOW visibility filtering.
    ///
    /// # Presentation boundary (host path)
    /// When `presentation_frame` is set, the **main unit mesh pass** iterates
    /// `PresentationFrame::unit_render_inputs()` only:
    /// position / orientation / team / model_key / selected / selection_radius /
    /// aliveness / engine_bridged / **fow_visibility** / shell FOW bypass —
    /// all snapshot-owned.
    ///
    /// Remaining residuals (not unit identity; see mesh_asset_resolve residual notes):
    /// - live fallback when no presentation frame is set (boot/loading)
    /// - mesh asset resolve: GraphicsSystem cache + AssetManager + filesystem residual
    ///   (`assets::mesh_asset_resolve`); deferred load budget still incremental
    /// - terrain / cell-grid FOW overlay (not unit mesh identity)
    ///
    /// Do **not** re-read live position/orientation/health/team/selected/model_key/FOW
    /// when presentation owns those fields.
    pub(super) fn collect_render_items(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        allow_sync_model_loads: bool,
        deferred_model_load_budget: &mut usize,
        delta_time: f32,
        direct_scene_candidates: &mut Vec<FrozenDirectDrawableSceneCandidate>,
    ) -> Result<()> {
        let collect_started = Instant::now();
        let object_ids_started = Instant::now();
        // Snapshot ownership: when presentation is present, drive the main unit
        // mesh pass from unit_render_inputs (no live object identity / FOW re-read).
        // Keep frame installed for post-collect execute residual (minimap/shell/heightmap).
        let presentation = self.presentation_frame.clone();
        let visual_plans: Vec<crate::presentation_frame::FrozenWeaponVisualDispatchPlan> =
            presentation
                .as_ref()
                .map(|frame| {
                    frame
                        .events
                        .iter()
                        .filter_map(|event| match event {
                            crate::presentation_frame::PresentationEvent::WeaponDischarged {
                                visual_plan,
                                ..
                            } => visual_plan.clone().filter(|plan| plan.is_valid()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
        let presentation_unit_pass = presentation.is_some();
        // Reset live-identity residual each collect; presentation path must stay at 0.
        self.debug_last_live_unit_identity_reads = 0;
        self.debug_last_presentation_live_fallback_reads = 0;
        // Shell FOW bypass from snapshot when available (no live GameLogic re-read).
        let bypass_fow = presentation
            .as_ref()
            .map(|p| p.fow_shell_bypass)
            .unwrap_or(false);

        // Snapshot-owned unit inputs for the main mesh pass (empty when no frame).
        let mut unit_inputs: Vec<crate::presentation_frame::UnitRenderInput> =
            if let Some(ref pres) = presentation {
                pres.unit_render_inputs()
            } else {
                Vec::new()
            };

        trace!(
            "collect_render_items processing {} units (presentation_unit_pass={})",
            unit_inputs.len(),
            presentation_unit_pass
        );

        if allow_sync_model_loads {
            unit_inputs.sort_by_key(|u| u.id.0);
        } else {
            // Distance sort from snapshot positions only — no live transform re-read.
            unit_inputs.sort_by(|a, b| {
                let da = a.position.distance_squared(camera_position);
                let db = b.position.distance_squared(camera_position);
                da.partial_cmp(&db)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.id.cmp(&b.id))
            });
        }
        let object_ids_elapsed = object_ids_started.elapsed();

        let mut alive_objects = 0usize;
        let mut fow_filtered = 0usize;
        let mut frustum_culled = 0usize;
        let mut model_missing = 0usize;
        // Wave 213: presentation FOW only — no live shroud batch / empty map residual.
        let visibility_started = Instant::now();
        let visibility_elapsed = visibility_started.elapsed();

        let view_proj = *projection_matrix * *view_matrix;
        let frustum_planes = extract_frustum_planes(&view_proj);

        let mut render_model_load_elapsed = Duration::ZERO;
        let mut render_item_build_elapsed = Duration::ZERO;
        // A single C++ Drawable can own multiple source Draw modules and each
        // module can produce many meshes. `renderOneObject` refreshes its
        // clear timestamp once for the accepted Drawable, so retain one full
        // binding key per collection rather than one record per mesh.
        let mut direct_scene_candidate_bindings = HashSet::new();

        // --- Main unit mesh pass: presentation-owned inputs only ---
        for u in unit_inputs {
            // engine_bridged already filtered in unit_render_inputs; keep guard.
            if u.engine_bridged {
                continue;
            }
            let object_id = u.id;
            // `UnitRenderInput::world_matrix` already applies the frozen
            // Object INI asset scale.  Applying it again here made every
            // non-1.0 object scale quadratically in the live WGPU pass.
            let world_matrix = gameplay_to_render_transform(u.world_matrix());
            // Presentation has already selected the exact source-authored
            // models *and animation state* from every ConditionState Draw
            // module. Never reconstruct either from combat bits or mesh-name
            // suffixes here: doing so can turn an exact damaged/construction
            // W3D key into guessed art or a guessed visibility channel.
            // Wave 501: deployed + radar dish bits included in stamp helper.
            // Wave 503: construction scaffold bits included in stamp helper.
            // Wave 504: GARRISONED bit included in stamp helper.
            // Wave 505: parachuting/jetexhaust/using-weapon bits included in stamp helper.
            // Wave 506: weaponset veterancy bits included in stamp helper.
            // Wave 507: OVER_WATER + transport RIDER bits included in stamp helper.
            // Wave 508: body-damage / DISGUISED / STUNNED bits included in stamp helper.
            // Wave 509: TOPPLED / FREEFALL / NIGHT / SNOW bits included in stamp helper.
            // Wave 510: CAPTURED / LOADED / POWER_PLANT_UPGRADED bits included in stamp helper.
            // Wave 511: BURNED / AFLAME / SPECIAL_CHEERING / CARRYING bits included in stamp helper.
            // Wave 513: JAMMED / DYING / RELOADING / PACKING / UNPACKING bits included in stamp helper.
            // Wave 515: RAISING_FLAG (surrendered) bit included in stamp helper.
            let template_name_owned = u.template_name.clone();
            let selection_radius = u.selection_radius;
            let snapshot_fow = Some(u.fow_visibility);
            let selection_flash_intensity = u.selection_flash_intensity();
            // Wave 499: defector_flash folded into selection_flash_intensity(); poison via apply_poison_tint.
            let team_color = u.team_color;
            let selection_flash_color = u.selection_flash_color_rgba();

            // `UnitRenderInput::from_renderable` normalizes old snapshots to
            // one module. Keep the same compatibility at this boundary for
            // direct test/boot inputs which still provide only `model_key`.
            let mut draw_models = if u.draw_models.is_empty() {
                (!u.model_key.trim().is_empty())
                    .then(|| crate::assets::AuthoredDrawModel {
                        module_index: 0,
                        model_key: u.model_key.clone(),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                u.draw_models.clone()
            };
            #[cfg(feature = "game_client")]
            if draw_models.is_empty() {
                if let Some(spec) =
                    game_client::core::game_client::presentation_specialized_draw_snapshot(
                        object_id.0,
                    )
                {
                    if spec.is_debris() && !spec.model_name.trim().is_empty() {
                        draw_models.push(crate::assets::AuthoredDrawModel {
                            module_index: 0,
                            model_key: spec.model_name.clone(),
                            ..Default::default()
                        });
                    }
                }
            }
            #[cfg(feature = "game_client")]
            if game_client::core::game_client::presentation_specialized_draw_snapshot(object_id.0)
                .is_some_and(|spec| spec.is_science_hidden())
            {
                continue;
            }

            alive_objects += 1;

            if draw_models.is_empty() {
                // Every source selection deliberately suppressed this object.
                // It is not a missing template that may borrow pristine art.
                model_missing += 1;
                continue;
            }

            // FOW never-explored skip: presentation path uses snapshot only.
            // Wave 213: unit inputs always carry presentation FOW snapshot.
            let fow_visibility = {
                let snap_vis =
                    snapshot_fow.expect("unit_render_inputs always stamp fow_visibility");
                if !bypass_fow && !snap_vis.should_render() {
                    fow_filtered += 1;
                    trace!(
                        "Skipping object {} - never explored (presentation FOW) by player {}",
                        object_id, self.current_player_id
                    );
                    continue;
                }
                if bypass_fow {
                    ObjectVisibility::FULLY_VISIBLE
                } else {
                    snap_vis
                }
            };

            for draw_model in draw_models {
                let draw_module_index = draw_model.module_index;
                // A queued v4 visual record is a one-shot candidate. Remove
                // it before culling/model resolution so an unavailable mesh
                // cannot retain stale saved playback for a later retry.
                let pending_client_drawable_restore = self
                    .pending_client_drawable_imports
                    .remove(&(object_id.0, draw_module_index));
                // C++ applies static Draw directives first, then broadcasts
                // weapon clip feedback in slot order. Preserve that exact
                // ordering before the HLOD resolver handles last-write wins.
                let authored_subobject_visibility =
                    u.authored_subobject_visibility_for_draw_model(&draw_model);
                let authored_primary_turret = draw_model.primary_turret.clone();
                let model_name_owned = draw_model.model_key.clone();
                if model_name_owned.trim().is_empty() {
                    // An empty key cannot be an exact Draw submission.
                    model_missing += 1;
                    continue;
                }
                let world_position = world_matrix.w_axis.truncate();
                let model_name = model_name_owned.as_str();
                let template_name_for_cull = template_name_owned.as_str();
                let model_hint_owned = Some(model_name_owned.clone());
                let (cull_center, cull_radius) = self.resolve_object_world_cull_sphere(
                    graphics_system,
                    model_name,
                    template_name_for_cull,
                    selection_radius,
                    world_matrix,
                );
                if !world_sphere_in_expanded_frustum(
                    &frustum_planes,
                    cull_center,
                    cull_radius,
                    camera_position,
                ) {
                    frustum_culled += 1;
                    continue;
                }

                // C++ `RTS3DScene::Visibility_Check` first accepts the
                // RenderObj's frustum sphere, then removes a direct Drawable
                // that is source-hidden (`m_hidden || m_hiddenByStealth`) or
                // whose GameClient update marked it fully obscured. The
                // immutable sidecar was captured before this execute call;
                // do not query or mutate GameClient here, and do not turn
                // this into the later `renderOneObject` clear-frame/material
                // decision.
                if frozen_direct_candidate_is_scene_culled(
                    &self.presentation_direct_shroud_states,
                    self.presentation_direct_shroud_host_epoch,
                    object_id,
                    u.drawable_shroud,
                ) {
                    fow_filtered += 1;
                    continue;
                }

                let model_hint = model_hint_owned.as_deref().or(Some(model_name));

                let model_load_started = Instant::now();
                let render_model_load_result = Self::ensure_render_model_loaded(
                    graphics_system,
                    template_name_for_cull,
                    model_name,
                    allow_sync_model_loads,
                    deferred_model_load_budget,
                );
                render_model_load_elapsed += model_load_started.elapsed();

                let render_item_build_started = Instant::now();
                match render_model_load_result {
                    RenderModelLoadResult::Ready(w3d_model) => {
                        let render_item_count_before_model = self.render_items.len();
                        {
                            let visibility = fow_visibility;

                            let (animation_binding, anim_frame, weapon_controls) = self
                                .advance_authored_draw_animation(
                                    object_id,
                                    draw_module_index,
                                    &w3d_model,
                                    template_name_owned.as_str(),
                                    &draw_model,
                                    delta_time,
                                    pending_client_drawable_restore,
                                    &visual_plans,
                                );

                            // C++ `HLodClass::Update_Sub_Object_Transforms`
                            // samples its additional-model parent bones after
                            // the same turret/recoil controls that position
                            // the rigid mesh children below. Retain the exact
                            // aggregate poses before the mesh loop so every
                            // resolvable external child sees that state once.
                            let aggregate_poses = w3d_model
                                .aggregate_attachment_poses_for_primary_turret_and_weapon_controls(
                                    animation_binding.as_ref(),
                                    anim_frame,
                                    &authored_primary_turret,
                                    u.turret_angle_deg,
                                    u.turret_pitch_deg,
                                    &weapon_controls,
                                );
                            let has_source_aggregate_attachments = aggregate_poses
                                .as_ref()
                                .is_some_and(|poses| !poses.is_empty());

                            for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                                let mut material = mesh.material.clone();

                                if material.texture_name.is_none() {
                                    if let Some(asset_manager_arc) =
                                        crate::assets::get_asset_manager()
                                    {
                                        if let Ok(asset_manager) = asset_manager_arc.lock() {
                                            if let Some(obj_def) = asset_manager
                                                .resolve_object_definition(
                                                    &template_name_owned,
                                                    // A W3D basename is not an Object INI identity:
                                                    // sharing it must never borrow a different
                                                    // faction/condition-state texture definition.
                                                    None,
                                                )
                                            {
                                                if let Some(texture_from_ini) =
                                                    obj_def.get_primary_texture()
                                                {
                                                    material.texture_name =
                                                        Some(texture_from_ini.to_string());
                                                    trace!(
                                                        "WW3D material fallback: object {} ('{}') -> texture {}",
                                                        object_id,
                                                        template_name_owned,
                                                        texture_from_ini
                                                    );
                                                } else if self.missing_ini_objects.insert(format!(
                                                    "{}::texture",
                                                    template_name_owned
                                                )) {
                                                    debug!(
                                                        "WW3D assets: INI definition for '{}' defines no textures",
                                                        template_name_owned
                                                    );
                                                }
                                            } else if self
                                                .missing_ini_objects
                                                .insert(template_name_owned.clone())
                                            {
                                                debug!(
                                                    "WW3D assets: no INI definition for '{}' (model hint: {:?})",
                                                    template_name_owned, model_hint
                                                );
                                            }
                                        }
                                    }
                                }

                                // HLOD rigid children resolve through the source-authored
                                // `HLOD.Name.MeshName -> BoneIndex` record. One rigid HLOD
                                // uses the original game's constructor-selected static level;
                                // separately resolved aggregates are collected after this
                                // parent pass. Proxies, multiple HLODs, and unresolved selected
                                // rigid children remain intentionally non-rendering rather than
                                // drawing every source group.
                                // C++ `HLodClass::Update_Sub_Object_Transforms` only walks
                                // created LOD children (`hlod.cpp:3236-3245`). A failed bone
                                // bind is not an identity-local draw — that was the floating
                                // shard path. Meshes outside the constructor-selected LOD
                                // fail `rigid_hlod_subobject_for_mesh` and are skipped here.
                                let Some((mesh_local_transform, animation_visible)) = w3d_model
                                    .mesh_local_transform_and_visibility_for_primary_turret_and_weapon_controls(
                                        mesh_idx,
                                        animation_binding.as_ref(),
                                        anim_frame,
                                        &authored_primary_turret,
                                        u.turret_angle_deg,
                                        u.turret_pitch_deg,
                                        &weapon_controls,
                                    )
                                else {
                                    continue;
                                };
                                if !animation_visible {
                                    continue;
                                }
                                // The frozen active Draw state owns these
                                // directives. Resolve them only through the
                                // model's retained single-HLOD child records;
                                // missing/unsupported records leave this mesh
                                // unchanged rather than guessing a suffix.
                                let authored_visible = w3d_model
                                    .mesh_visible_for_authored_subobject_directives(
                                        mesh_idx,
                                        &authored_subobject_visibility,
                                    );
                                // `handleClientRecoil` runs after C++ static
                                // ModelCondition Hide/Show. It can therefore
                                // reveal or hide only the exact first muzzle
                                // child on its pivot; selected HAnim invisibility
                                // above remains authoritative.
                                if !w3d_model
                                    .muzzle_flash_visibility_override_for_mesh(
                                        mesh_idx,
                                        &weapon_controls,
                                    )
                                    .unwrap_or(authored_visible)
                                {
                                    continue;
                                }
                                if !transform_is_reasonable_for_mesh(mesh_local_transform) {
                                    let key = format!(
                                        "{}::{}::{}",
                                        template_name_owned, model_name, mesh.name
                                    );
                                    if self.debug_warned_bad_mesh_transforms.insert(key.clone()) {
                                        warn!(
                                            "Invalid mesh local transform for '{}': template='{}' model='{}' mesh='{}'; skipping (C++ does not identity-draw failed HLOD binds)",
                                            key, template_name_owned, model_name, mesh.name
                                        );
                                    }
                                    continue;
                                }
                                let mut render_item = RenderItem::new(
                                    object_id,
                                    model_name.to_string(),
                                    mesh_idx,
                                    world_position,
                                    world_matrix,
                                    &material,
                                    Self::render_pass_for_material(&material),
                                );
                                render_item.set_mesh_local_transform(mesh_local_transform);
                                stamp_skinned_hierarchy_bind_pose(&mut render_item, mesh);
                                render_item.distance = world_position.distance(camera_position);
                                render_item.apply_frozen_presentation_visuals(
                                    visibility,
                                    selection_flash_intensity,
                                    team_color,
                                    u.poison_tinted,
                                    selection_flash_color,
                                );

                                render_item.apply_status_tint(u.status_tint);
                                render_item.set_presentation_opacity(u.presentation_opacity);
                                render_item
                                    .apply_heat_vision_second_pass(u.second_material_pass_opacity);
                                render_item.apply_house_color_livery(&mesh.name);
                                render_item.animation_frame = anim_frame;
                                render_item.animation_binding = animation_binding.clone();
                                #[cfg(feature = "game_client")]
                                if let Some(spec) =
                                    game_client::core::game_client::presentation_specialized_draw_snapshot(
                                        object_id.0,
                                    )
                                {
                                    if let Some(uv) = spec.tread_uv_for_mesh(&mesh.name) {
                                        render_item.uv_offset_override =
                                            Some(glam::Vec2::new(uv[0], uv[1]));
                                    }
                                }

                                self.render_items.push(render_item);
                            }

                            // `HLodClass` renders each AdditionalModel after
                            // its selected parent LOD. The independently
                            // created child keeps the parent Drawable's world
                            // transform/FOW ownership, while its exact HTree
                            // attachment pose was frozen above with the same
                            // HAnim, turret, and recoil controls as the rigid
                            // source children.
                            if let Some(aggregate_poses) = aggregate_poses {
                                let fallback_parent_material = W3DMaterial::default();
                                let parent_material = w3d_model
                                    .meshes
                                    .first()
                                    .map(|mesh| &mesh.material)
                                    .unwrap_or(&fallback_parent_material);
                                let mut aggregate_parent_item = RenderItem::new(
                                    object_id,
                                    model_name.to_string(),
                                    0,
                                    world_position,
                                    world_matrix,
                                    parent_material,
                                    Self::render_pass_for_material(parent_material),
                                );
                                aggregate_parent_item.apply_frozen_presentation_visuals(
                                    visibility,
                                    selection_flash_intensity,
                                    team_color,
                                    u.poison_tinted,
                                    selection_flash_color,
                                );

                                aggregate_parent_item.apply_status_tint(u.status_tint);
                                aggregate_parent_item.apply_house_color_livery(
                                    w3d_model
                                        .meshes
                                        .first()
                                        .map(|m| m.name.as_str())
                                        .unwrap_or(""),
                                );
                                aggregate_parent_item
                                    .set_presentation_opacity(u.presentation_opacity);
                                aggregate_parent_item
                                    .apply_heat_vision_second_pass(u.second_material_pass_opacity);
                                self.render_items.extend(
                                    super::hlod_aggregate_render::collect_cached_hlod_aggregate_render_items(
                                        graphics_system,
                                        &aggregate_parent_item,
                                        &aggregate_poses,
                                        camera_position,
                                    ),
                                );
                            }

                            // This is intentionally after every source W3D
                            // mesh and aggregate has had a chance to append a
                            // real item, but before fallback/debug geometry.
                            // It is the narrow Main-only seam for C++ scene
                            // timing: a frustum-accepted direct Drawable
                            // reaches renderOneObject only if it actually
                            // submitted an eligible source render item.
                            if self.render_items.len() > render_item_count_before_model {
                                if let Some(candidate) = frozen_direct_scene_candidate(
                                    &self.presentation_direct_shroud_states,
                                    self.presentation_direct_shroud_host_epoch,
                                    object_id,
                                    u.drawable_shroud,
                                ) {
                                    let binding_key = (
                                        candidate.host_epoch,
                                        candidate.object_id.0,
                                        candidate.drawable_id,
                                        candidate.binding_generation,
                                    );
                                    if direct_scene_candidate_bindings.insert(binding_key) {
                                        direct_scene_candidates.push(candidate);
                                    }
                                }
                            }
                            if self.render_items.len() > render_item_count_before_model {
                                trace!(
                                    "Object {} will render with FOW alpha={}, explored={}",
                                    object_id, visibility.visibility_alpha, visibility.is_explored
                                );
                                render_item_build_elapsed += render_item_build_started.elapsed();
                                continue; // Skip the fallback path
                            }
                        }

                        if w3d_model.meshes.is_empty() {
                            self.debug_last_zero_mesh_models += 1;
                            // Ready-but-empty is a missing visual, not a successful draw.
                            model_missing += 1;
                            if self.debug_last_missing_model_samples.len() < 16 {
                                self.debug_last_missing_model_samples.push(format!(
                                    "{}:{} [zero-mesh Ready]",
                                    template_name_owned, model_name
                                ));
                            }
                        }
                        // Animation/HLOD produced no items. Do not skip the
                        // fallback cube — the previous meshes.is_empty() gate
                        // hid every real W3D that failed its pose.
                        // Fall through to fallback cube below (same as Failed path).

                        if Self::missing_model_debug_cubes_enabled()
                            && !real_w3d_name_resolved(model_name)
                        {
                            if let Some(fallback_model) =
                                graphics_system.get_model_or_fallback("__fallback_cube__")
                            {
                                if !fallback_model.meshes.is_empty() {
                                    let fallback_mesh = &fallback_model.meshes[0];
                                    let mut render_item = RenderItem::new(
                                        object_id,
                                        "__fallback_cube__".to_string(),
                                        0,
                                        world_position,
                                        world_matrix,
                                        &fallback_mesh.material,
                                        RenderPass::ForwardOpaque,
                                    );
                                    render_item.distance = world_position.distance(camera_position);
                                    render_item.apply_frozen_presentation_visuals(
                                        fow_visibility,
                                        selection_flash_intensity,
                                        team_color,
                                        u.poison_tinted,
                                        selection_flash_color,
                                    );

                                    render_item.apply_status_tint(u.status_tint);
                                    render_item.set_presentation_opacity(u.presentation_opacity);
                                    render_item.apply_heat_vision_second_pass(
                                        u.second_material_pass_opacity,
                                    );

                                    self.render_items.push(render_item);
                                }
                            }
                        }
                    }
                    RenderModelLoadResult::SkippedByBudget => {
                        self.debug_last_model_budget_skips += 1;
                        if self.debug_last_missing_model_samples.len() < 16 {
                            self.debug_last_missing_model_samples
                                .push(format!("{}:{} [budget]", template_name_owned, model_name));
                        }
                        model_missing += 1;
                    }
                    RenderModelLoadResult::Failed => {
                        if self.debug_last_missing_model_samples.len() < 16 {
                            // Prefer presentation/live-resolved model hint (no re-read of Object).
                            let explicit = model_hint_owned.as_deref().unwrap_or("");
                            self.debug_last_missing_model_samples.push(format!(
                                "{}:{} explicit_model={}",
                                template_name_owned,
                                model_name,
                                if explicit.is_empty() {
                                    "<none>"
                                } else {
                                    explicit
                                }
                            ));
                        }
                        model_missing += 1;

                        if Self::missing_model_debug_cubes_enabled()
                            && !real_w3d_name_resolved(model_name)
                        {
                            if let Some(fallback_model) =
                                graphics_system.get_model_or_fallback("__fallback_cube__")
                            {
                                if !fallback_model.meshes.is_empty() {
                                    let fallback_mesh = &fallback_model.meshes[0];
                                    let mut render_item = RenderItem::new(
                                        object_id,
                                        "__fallback_cube__".to_string(),
                                        0,
                                        world_position,
                                        world_matrix,
                                        &fallback_mesh.material,
                                        RenderPass::ForwardOpaque,
                                    );
                                    render_item.distance = world_position.distance(camera_position);
                                    render_item.apply_frozen_presentation_visuals(
                                        fow_visibility,
                                        selection_flash_intensity,
                                        team_color,
                                        u.poison_tinted,
                                        selection_flash_color,
                                    );

                                    render_item.apply_status_tint(u.status_tint);
                                    render_item.set_presentation_opacity(u.presentation_opacity);
                                    render_item.apply_heat_vision_second_pass(
                                        u.second_material_pass_opacity,
                                    );

                                    self.render_items.push(render_item);
                                }
                            }
                        }
                    }
                }
                render_item_build_elapsed += render_item_build_started.elapsed();
            }
        }

        // Presentation projectile mesh residual: enqueue model_key instances without
        // live GameLogic. Fail-closed when W3D missing (trail pack still primary).
        if presentation_unit_pass {
            let proj_inputs = self
                .presentation_frame
                .as_ref()
                .map(|p| p.projectile_render_inputs())
                .unwrap_or_default();
            for p in proj_inputs {
                let model_name = p.model_key.as_str();
                if model_name.is_empty() {
                    continue;
                }
                let world_matrix = gameplay_to_render_transform(p.world_matrix());
                let world_position = world_matrix.w_axis.truncate();
                let template_name = if p.projectile_object_name.is_empty() {
                    model_name
                } else {
                    p.projectile_object_name.as_str()
                };
                let render_model_load_result = Self::ensure_render_model_loaded(
                    graphics_system,
                    template_name,
                    model_name,
                    allow_sync_model_loads,
                    deferred_model_load_budget,
                );
                match render_model_load_result {
                    RenderModelLoadResult::Ready(w3d_model) => {
                        if w3d_model.meshes.is_empty() {
                            model_missing += 1;
                            continue;
                        }
                        for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                            let material = mesh.material.clone();
                            let Some(mesh_local_transform) = w3d_model
                                .mesh_local_transform_for_animation(mesh_idx, 0, 0.0)
                                .filter(|transform| transform_is_reasonable_for_mesh(*transform))
                            else {
                                continue;
                            };
                            let mut render_item = RenderItem::new_presentation_projectile(
                                p.id,
                                model_name.to_string(),
                                mesh_idx,
                                world_position,
                                world_matrix,
                                &material,
                                Self::render_pass_for_material(&material),
                            );
                            render_item.set_mesh_local_transform(mesh_local_transform);
                            stamp_skinned_hierarchy_bind_pose(&mut render_item, mesh);
                            self.render_items.push(render_item);
                        }
                        alive_objects += 1;
                    }
                    _ => {
                        model_missing += 1;
                    }
                }
            }
        }

        self.debug_last_alive_objects = alive_objects;
        self.debug_last_fow_filtered = fow_filtered;
        self.debug_last_frustum_culled = frustum_culled;
        self.debug_last_model_missing = model_missing;
        if self.frame_number <= 30 || self.frame_number.is_multiple_of(300) {
            info!(
                "Collected {} render items (alive={} fow_skip={} frustum_skip={} model_missing={}) player={}",
                self.render_items.len(),
                self.debug_last_alive_objects,
                self.debug_last_fow_filtered,
                self.debug_last_frustum_culled,
                self.debug_last_model_missing,
                self.current_player_id
            );
        }
        let collect_elapsed = collect_started.elapsed();
        if collect_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || object_ids_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || visibility_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || render_model_load_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || render_item_build_elapsed >= PROFILE_STEP_LOG_THRESHOLD
        {
            debug!(
                "Render collection breakdown: total={:?} ids={:?} visibility={:?} model_load={:?} item_build={:?} alive={} filtered={} missing={} items={}",
                collect_elapsed,
                object_ids_elapsed,
                visibility_elapsed,
                render_model_load_elapsed,
                render_item_build_elapsed,
                self.debug_last_alive_objects,
                self.debug_last_fow_filtered,
                self.debug_last_model_missing,
                self.render_items.len()
            );
        }

        Ok(())
    }

    pub(super) fn resolve_object_world_cull_sphere(
        &mut self,
        graphics_system: &GraphicsSystem,
        model_name: &str,
        template_name: &str,
        selection_radius: f32,
        world_matrix: Mat4,
    ) -> (Vec3, f32) {
        let mut model_bounds = self.model_cull_bounds_cache.get(model_name).copied();
        if model_bounds.is_none() {
            // Bounds are cached under the resolved presentation model.  Using
            // a pristine template cache entry here would make a damaged or
            // construction state inherit the wrong cull sphere before its
            // exact mesh is loaded.
            let source = graphics_system.get_model(model_name);
            if let Some(model) = source {
                model_bounds = Self::model_local_cull_bounds(model.as_ref());
                if let Some(bounds) = model_bounds {
                    self.model_cull_bounds_cache
                        .insert(model_name.to_string(), bounds);
                }
            }
        }

        let world_scale = Self::world_cull_scale(world_matrix);
        let fallback_center = world_matrix.w_axis.truncate();
        match model_bounds {
            Some((local_center, local_radius)) => {
                // Prefer the W3D AABB. selection_radius 0→10 is only a lower
                // bound once real mesh extents exist — never the sole sphere.
                let fallback_radius = selection_radius.max(Self::structure_cull_fallback_radius(
                    template_name,
                    model_name,
                ));
                let world_center = world_matrix.transform_point3(local_center);
                let world_radius = Self::scaled_world_cull_radius(
                    Some(local_radius),
                    fallback_radius,
                    world_scale,
                );
                (world_center, world_radius)
            }
            None => {
                // No cached AABB: a 10-unit guess at the object origin culls
                // AmericaCommandCenter (1362) before the mesh can load, so the
                // bounds never populate. Pass through until the W3D AABB exists.
                (fallback_center, f32::INFINITY)
            }
        }
    }

    /// Largest affine-axis scale used to convert local W3D bounds into the
    /// presentation world's cull sphere. A valid authored visual replacement
    /// may be smaller than one (for example, a disguise template at 0.7), so
    /// do not inflate it to unit scale. Degenerate/non-finite matrices fail
    /// safely to unit scale instead of producing a NaN/zero cull radius.
    pub(super) fn world_cull_scale(world_matrix: Mat4) -> f32 {
        let axis_scales = [
            world_matrix.x_axis.truncate().length(),
            world_matrix.y_axis.truncate().length(),
            world_matrix.z_axis.truncate().length(),
        ];
        if axis_scales
            .iter()
            .all(|scale| scale.is_finite() && *scale > 0.0)
        {
            axis_scales.into_iter().fold(0.0, f32::max)
        } else {
            1.0
        }
    }

    /// Scale both the exact local W3D bounds and their cull fallback by the
    /// same current Drawable transform. The fallback remains a lower bound;
    /// it just lives in world space rather than unscaled model space.
    pub(super) fn scaled_world_cull_radius(
        local_radius: Option<f32>,
        fallback_radius: f32,
        world_scale: f32,
    ) -> f32 {
        let scaled_fallback_radius = fallback_radius * world_scale;
        local_radius
            .map(|local_radius| (local_radius * world_scale).max(scaled_fallback_radius))
            .unwrap_or(scaled_fallback_radius)
    }

    /// Minimum cull sphere radius when model bounds are not yet cached.
    /// Infantry/vehicles stay at 10; faction structures need a much larger pad
    /// so first-frame frustum tests do not drop CCs before W3D load.
    pub(super) fn structure_cull_fallback_radius(template_name: &str, model_name: &str) -> f32 {
        let blob = format!(
            "{}|{}",
            template_name.to_ascii_lowercase(),
            model_name.to_ascii_lowercase()
        );
        if blob.contains("commandcenter")
            || blob.contains("command_center")
            || blob.contains("cmdhq")
            || blob.contains("conyard")
            || blob.contains("warfactory")
            || blob.contains("war_factory")
            || blob.contains("warfact")
            || blob.contains("barracks")
            || blob.contains("powerplant")
            || blob.contains("power_plant")
            || blob.contains("pwrplant")
            || blob.contains("supplycenter")
            || blob.contains("supply_center")
            || blob.contains("supplyct")
            || blob.contains("supplystash")
            || blob.contains("supcent")
            || blob.contains("armsdealer")
            || blob.contains("tunnel")
            || blob.contains("stinger")
            || blob.contains("patriot")
        {
            64.0
        } else {
            10.0
        }
    }

    pub(super) fn model_local_cull_bounds(model: &crate::assets::W3DModel) -> Option<(Vec3, f32)> {
        if let Some(bounds) =
            Self::aabb_to_cull_sphere(model.bounding_box_min, model.bounding_box_max)
        {
            return Some(bounds);
        }
        // Bind-pose can fail for every HLOD child (container/identity mismatch),
        // leaving the stored AABB at the zero sentinel. Union mesh vertices in
        // their baked/local transform so a CC is not a radius-10 guess.
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let mut any = false;
        for mesh in &model.meshes {
            let xform = if mesh.transform.is_finite() {
                mesh.transform
            } else {
                Mat4::IDENTITY
            };
            for vertex in &mesh.vertices {
                let pos = xform.transform_point3(Vec3::from_array(vertex.position));
                if !pos.is_finite() {
                    continue;
                }
                min = min.min(pos);
                max = max.max(pos);
                any = true;
            }
        }
        if any {
            Self::aabb_to_cull_sphere(min, max)
        } else {
            None
        }
    }

    fn aabb_to_cull_sphere(min: Vec3, max: Vec3) -> Option<(Vec3, f32)> {
        if !min.is_finite() || !max.is_finite() {
            return None;
        }
        if min.x > max.x || min.y > max.y || min.z > max.z {
            return None;
        }
        let center = (min + max) * 0.5;
        let extents = (max - min) * 0.5;
        let radius = extents.length();
        if radius.is_finite() && radius > 0.0 {
            Some((center, radius))
        } else {
            None
        }
    }

    pub(super) fn ensure_render_model_loaded(
        graphics_system: &mut GraphicsSystem,
        template_name: &str,
        model_name: &str,
        allow_sync_model_loads: bool,
        deferred_model_load_budget: &mut usize,
    ) -> RenderModelLoadResult {
        use crate::assets::mesh_asset_resolve::{
            MeshResolveResult, PLACEHOLDER_MODEL_KEY, canonical_model_key,
            resolve_mesh_for_model_key,
        };

        static STARTUP_MODEL_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);
        let trace_this_attempt = !allow_sync_model_loads
            && STARTUP_MODEL_TRACE_COUNT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    (count < 64).then_some(count + 1)
                })
                .is_ok();

        // `model_name` is an opaque, already-selected presentation key.  Only
        // normalize its filename spelling; never remap it through a template
        // alias or condition suffix table at render time.
        let resolved_key = canonical_model_key(model_name);
        if resolved_key.is_empty() {
            return RenderModelLoadResult::Failed;
        }

        if let Some(model) = graphics_system.get_model(&resolved_key).cloned() {
            if trace_this_attempt {
                info!(
                    "Startup model load: cache hit template='{}' model='{}'",
                    template_name, resolved_key
                );
            }
            return RenderModelLoadResult::Ready(model);
        }
        if resolved_key != model_name {
            if let Some(model) = graphics_system.get_model(model_name).cloned() {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
                return RenderModelLoadResult::Ready(model);
            }
        }
        if !allow_sync_model_loads && *deferred_model_load_budget == 0 {
            if trace_this_attempt {
                info!(
                    "Startup model load: skipped by budget template='{}' model='{}'",
                    template_name, resolved_key
                );
            }
            return RenderModelLoadResult::SkippedByBudget;
        }

        if !allow_sync_model_loads {
            *deferred_model_load_budget -= 1;
        }

        let requested_model_name = resolved_key.clone();
        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            let loaded_model = match asset_manager_arc.lock() {
                Ok(mut asset_manager) => {
                    if trace_this_attempt {
                        info!(
                            "Startup model load: template='{}' exact_model='{}' requested='{}'",
                            template_name, model_name, requested_model_name
                        );
                    }

                    match asset_manager.load_w3d_model(&requested_model_name) {
                        Ok(model) => Some(model),
                        Err(err) => {
                            // Do not turn a pristine model miss into a damaged,
                            // construction, snow, or faction variant.  Those are
                            // distinct retail W3D assets selected by C++
                            // ConditionState logic, not aliases.
                            warn!(
                                "Failed to load W3D model '{}' for object '{}': {}",
                                requested_model_name, template_name, err
                            );
                            None
                        }
                    }
                }
                Err(err) => {
                    if trace_this_attempt {
                        warn!(
                            "Startup model load: asset manager lock poisoned for template='{}' model='{}': {}",
                            template_name, model_name, err
                        );
                    }
                    None
                }
            };

            if let Some(model) = loaded_model {
                graphics_system.cache_model(requested_model_name.clone(), model.clone());
                if requested_model_name != resolved_key {
                    graphics_system.cache_model(resolved_key.clone(), model.clone());
                }
                if requested_model_name != model_name {
                    graphics_system.cache_model(model_name.to_string(), model.clone());
                }
                if trace_this_attempt {
                    info!(
                        "Startup model load: success template='{}' requested='{}'",
                        template_name, requested_model_name
                    );
                }
            }
        }

        let resolved = if let Some(model) = graphics_system.get_model(&resolved_key).cloned() {
            Some(model)
        } else if let Some(model) = graphics_system.get_model(model_name).cloned() {
            if model_name != resolved_key {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            Some(model)
        } else if let Some(model) = graphics_system.get_model(&requested_model_name).cloned() {
            if requested_model_name != resolved_key {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            Some(model)
        } else {
            // Mesh residual path: filesystem W3D (extracted/sample) or honesty placeholder.
            // use_placeholder only when debug cubes are enabled (production remains fail-closed
            // for missing retail meshes unless opt-in).
            let use_placeholder = Self::missing_model_debug_cubes_enabled();
            match resolve_mesh_for_model_key(&resolved_key, use_placeholder) {
                MeshResolveResult::Loaded {
                    model_key,
                    model,
                    source_path,
                } => {
                    if trace_this_attempt {
                        info!(
                            "Startup model load: residual resolve template='{}' key='{}' path={:?}",
                            template_name, model_key, source_path
                        );
                    }
                    graphics_system.cache_model(model_key.clone(), model.clone());
                    if model_key != model_name {
                        graphics_system.cache_model(model_name.to_string(), model.clone());
                    }
                    if model_key != resolved_key {
                        graphics_system.cache_model(resolved_key.clone(), model.clone());
                    }
                    Some(std::sync::Arc::new(model))
                }
                MeshResolveResult::Placeholder { model, .. } => {
                    // Cache under both placeholder sentinel and requested key for draw.
                    graphics_system.cache_model(PLACEHOLDER_MODEL_KEY.to_string(), model.clone());
                    if use_placeholder {
                        // Return Ready so the unit pass can draw the honest placeholder mesh.
                        graphics_system.cache_model(resolved_key.clone(), model.clone());
                        Some(std::sync::Arc::new(model))
                    } else {
                        None
                    }
                }
                MeshResolveResult::Missing { .. } => None,
            }
        };
        if trace_this_attempt && resolved.is_none() {
            warn!(
                "Startup model load: unresolved template='{}' model='{}' requested='{}'",
                template_name, model_name, requested_model_name
            );
        }
        resolved
            .map(RenderModelLoadResult::Ready)
            .unwrap_or(RenderModelLoadResult::Failed)
    }

    /// Drain submissions from the GameClient RenderBridge and convert them
    /// into `RenderItem`s so they flow through the existing ForwardPass.
    ///
    /// C++ parity: drawables submit to the WW3D scene during
    /// `GameClient::update()`; the render pipeline then consumes those
    /// submissions during `RenderPipeline::execute()`.
    #[cfg(feature = "game_client")]
    pub(super) fn append_frozen_mesh_ghost_scene(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        camera_position: Vec3,
        allow_sync_model_loads: bool,
        deferred_model_load_budget: &mut usize,
    ) {
        let Some(frame) = self.frozen_ghost_scene.clone() else {
            return;
        };
        if frame.snapshots.is_empty() {
            return;
        }

        // The frozen scene names are the only permitted asset keys. Resolve
        // each exact source through the ordinary cache admission path, but do
        // not substitute a placeholder when a ghost asset is unavailable.
        let mut model_names = frame
            .snapshots
            .iter()
            .filter(|snapshot| {
                matches!(
                    snapshot.render_object.render_object.class_id,
                    gamelogic::object::w3d_ghost_object::RenderObjectClass::Mesh
                        | gamelogic::object::w3d_ghost_object::RenderObjectClass::HLod
                )
            })
            .map(|snapshot| snapshot.render_object.render_object.name.clone())
            .collect::<Vec<_>>();
        model_names.sort_unstable();
        model_names.dedup();
        for model_name in &model_names {
            if graphics_system.get_model(model_name).is_some() {
                continue;
            }
            let _ = Self::ensure_render_model_loaded(
                graphics_system,
                model_name,
                model_name,
                allow_sync_model_loads,
                deferred_model_load_budget,
            );
        }

        let Some(scene) = crate::graphics::render_item::materialize_frozen_w3d_ghost_scene(
            &frame,
            |model_name| graphics_system.get_model(model_name).cloned(),
        ) else {
            return;
        };

        // C++ removes the parent RenderObj from the scene before inserting a
        // ghost snapshot. Preserve that ordering without touching projectile,
        // objectless, or other standalone client ownership domains.
        self.render_items.retain(|item| {
            !matches!(
                item.owner,
                RenderItemOwner::Object(object_id)
                    if scene.parent_suppression.suppresses(object_id)
            )
        });

        match game_client::drawable::evaluate_ghost_scene() {
            game_client::drawable::SceneShroudDecision::RenderGhostWithFogLight => {}
            _ => return,
        }
        for materialized in scene.items {
            let state = materialized.state;
            let model = materialized.asset;
            let color = state.argb_color_rgba();
            let color_rgb = Vec3::new(color[0], color[1], color[2]);
            let scale = Mat4::from_scale(Vec3::splat(state.object_scale));
            let world_position = state.world_transform.w_axis.truncate();
            for (mesh_index, mesh) in model.meshes.iter().enumerate() {
                let mesh_local_transform = if let Some(child) = state
                    .sub_objects
                    .iter()
                    .find(|child| child.name.eq_ignore_ascii_case(&mesh.name))
                {
                    if !child.visible {
                        continue;
                    }
                    child.local_transform
                } else {
                    let Some(transform) =
                        model.mesh_local_transform_for_animation(mesh_index, usize::MAX, 0.0)
                    else {
                        continue;
                    };
                    transform
                };
                if !transform_is_reasonable_for_mesh(mesh_local_transform) {
                    continue;
                }
                let mut material = mesh.material.clone();
                material.diffuse_color *= color_rgb;
                material.opacity *= color[3];
                let Some(mut item) = RenderItem::new_w3d_ghost(
                    state.clone(),
                    mesh_index,
                    &material,
                    RenderPass::Ghost,
                ) else {
                    continue;
                };
                item.set_mesh_local_transform(scale * mesh_local_transform);
                stamp_skinned_hierarchy_bind_pose(&mut item, mesh);
                item.distance = world_position.distance(camera_position);
                self.render_items.push(item);
            }
        }
    }

    /// Drain submissions from the GameClient RenderBridge and convert them
    #[cfg(feature = "game_client")]
    pub(super) fn drain_render_bridge_submissions(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        camera_position: Vec3,
        deferred_model_load_budget: &mut usize,
    ) {
        use game_client::render_bridge::get_render_bridge;

        // Freeze logic-owned W3D ghost mutations before taking the client
        // bridge lock. This retains the exact snapshot payload and stable
        // pooled-module identity without a live cross-subsystem borrow.
        let ghost_events = gamelogic::object::THE_W3D_GHOST_OBJECT_MANAGER
            .write()
            .map(|mut manager| manager.drain_scene_events())
            .unwrap_or_default();

        let mut bridge_guard = match get_render_bridge().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let bridge = match bridge_guard.as_mut() {
            Some(b) => b,
            None => return,
        };

        bridge.apply_ghost_scene_events(ghost_events);
        self.frozen_ghost_scene = Some(bridge.freeze_ghost_scene());

        // Live host never calls RenderBridge::begin_frame, so flush() has no
        // camera and would drain+drop pending. Pull them first so placement
        // ghosts (C++ InGameUI place-icons) reach graphics_system.
        let extra_pending = bridge.take_pending();
        bridge.flush();
        let mut submissions = bridge.drain_scene_submissions();
        for submission in extra_pending {
            let is_transparent = submission.transparent || submission.render_state.opacity < 1.0;
            submissions.push(game_client::render_bridge::DrainedDrawSubmission {
                submission,
                is_transparent,
                model_resolution: None,
            });
        }
        if submissions.is_empty() {
            return;
        }

        let submissions_count = submissions.len();
        let mut bridge_items_added = 0usize;

        for drained in submissions {
            let submission = drained.submission;
            let is_transparent = drained.is_transparent;
            let model_name = &submission.model_name;
            if model_name.is_empty() {
                continue;
            }
            // GameClient DrawableID and gameplay ObjectID are independent C++
            // identities.  Standalone drawables (tracers, placement previews,
            // FX) are legitimate client visuals, so preserve their distinct
            // ownership instead of either casting the ID or dropping them.
            let owner_object_id = submission.owner_object_id;
            // C++ objectless Drawables have no gameplay owner or direct
            // Drawable clear-frame state. Their optional
            // DrawableInfo::m_shroudStatusObjectID is resolved only against
            // this frozen presentation frame and retained on each resulting
            // render item. Never derive this branch from FOW alpha.
            let objectless_shroud = Self::frozen_objectless_drawable_shroud_for_submission(
                self.presentation_frame.as_ref(),
                &submission,
            );

            let render_pass = if is_transparent {
                RenderPass::ForwardTransparent
            } else {
                RenderPass::ForwardOpaque
            };

            let client_transform: Mat4 = submission.world_transform;
            let world_matrix = Mat4::from_cols_array_2d(&client_transform.to_cols_array_2d());
            let world_position = Vec3::new(
                world_matrix.w_axis.x,
                world_matrix.w_axis.y,
                world_matrix.w_axis.z,
            );

            let vis_alpha = submission.render_state.opacity;
            let fow_vis = ObjectVisibility {
                visibility_alpha: vis_alpha,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            };

            // `DrawSubmission` transports persistent SELECTED/render-state
            // flags and a combined generic emissive tint, but it has no
            // frozen Drawable selection-envelope intensity or poisoned tint
            // status. Those cannot be reconstructed from a selected bit (or
            // from a tint that may represent another effect), so the bridge
            // intentionally does not fabricate the presentation modifiers
            // applied by the direct frozen-unit path above.

            let load_result = Self::ensure_render_model_loaded(
                graphics_system,
                model_name,
                model_name,
                true,
                deferred_model_load_budget,
            );

            match load_result {
                RenderModelLoadResult::Ready(w3d_model) => {
                    {
                        // GameClient freezes its selected W3DModelDraw clip
                        // and normalized frame fraction into the bridge
                        // submission. Apply that exact state so raw W3D bit
                        // channels govern HLOD children here too.
                        let (animation_binding, anim_frame) = Self::bridge_draw_animation(
                            &w3d_model,
                            submission.animation_name.as_deref(),
                            submission.animation_time,
                        );

                        let bridge_subobject_visibility = submission
                            .sub_object_visibility
                            .iter()
                            .map(|visibility| {
                                crate::assets::ini_parser::AuthoredDrawSubobjectVisibility {
                                    name: visibility.sub_object_name.clone(),
                                    hidden: visibility.hidden,
                                }
                            })
                            .collect::<Vec<_>>();
                        let capture_bone_controls = submission
                            .bone_overrides
                            .iter()
                            .map(|override_state| {
                                (
                                    override_state.bone_index,
                                    Mat4::from_cols_array_2d(
                                        &override_state.transform.to_cols_array_2d(),
                                    ),
                                )
                            })
                            .collect::<Vec<_>>();
                        let bridge_house_color = submission
                            .legacy_render_object_color
                            .and_then(house_color_from_argb);

                        for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                            let Some((mesh_local_transform, mesh_visible)) = w3d_model
                                .mesh_local_transform_and_visibility_for_binding_and_capture_controls(
                                    mesh_idx,
                                    animation_binding.as_ref(),
                                    anim_frame,
                                    &capture_bone_controls,
                                )
                            else {
                                continue;
                            };
                            if !mesh_visible {
                                continue;
                            }
                            if !w3d_model.mesh_visible_for_authored_subobject_directives(
                                mesh_idx,
                                &bridge_subobject_visibility,
                            ) {
                                continue;
                            }
                            if !transform_is_reasonable_for_mesh(mesh_local_transform) {
                                continue;
                            }
                            let mut item = match owner_object_id {
                                Some(object_id) => RenderItem::new(
                                    crate::game_logic::ObjectId(object_id),
                                    model_name.clone(),
                                    mesh_idx,
                                    world_position,
                                    world_matrix,
                                    &mesh.material,
                                    render_pass,
                                ),
                                None => RenderItem::new_unbound_client_drawable(
                                    submission.drawable_id.0,
                                    model_name.clone(),
                                    mesh_idx,
                                    world_position,
                                    world_matrix,
                                    &mesh.material,
                                    render_pass,
                                ),
                            };
                            item.distance = world_position.distance(camera_position);
                            item.set_fow_visibility(fow_vis);
                            if let Some(state) = objectless_shroud {
                                item.set_frozen_objectless_drawable_shroud(state);
                            }
                            item.animation_frame = anim_frame;
                            item.animation_binding = animation_binding.clone();
                            item.capture_bone_controls = capture_bone_controls.clone();
                            item.set_mesh_local_transform(mesh_local_transform);
                            stamp_skinned_hierarchy_bind_pose(&mut item, mesh);
                            Self::attach_bridge_draw_metadata(&mut item, &submission);
                            if let Some(rgba) = bridge_house_color {
                                item.apply_house_color_livery_with(rgba, &mesh.name);
                            }
                            item.uv_offset_override =
                                Self::mesh_uv_override_for_submission(&submission, &mesh.name);
                            self.render_items.push(item);
                        }

                        // C++ `HLodClass::Update_Sub_Object_Transforms` also
                        // assigns the frozen parent HTree pose to every
                        // `AdditionalModels` entry.  Resolve those external
                        // render objects only from the strict cache; this
                        // bridge path already owns the exact capture controls
                        // frozen by GameClient, so aggregate bones receive the
                        // same palette/control state as the parent meshes.
                        let aggregate_poses = w3d_model
                            .aggregate_attachment_poses_for_binding_and_capture_controls(
                                animation_binding.as_ref(),
                                anim_frame,
                                &capture_bone_controls,
                            );
                        let has_source_aggregate_attachments = aggregate_poses
                            .as_ref()
                            .is_some_and(|poses| !poses.is_empty());
                        if let Some(aggregate_poses) = aggregate_poses {
                            let fallback_parent_material = W3DMaterial::default();
                            let parent_material = w3d_model
                                .meshes
                                .first()
                                .map(|mesh| &mesh.material)
                                .unwrap_or(&fallback_parent_material);
                            let mut aggregate_parent_item = match owner_object_id {
                                Some(object_id) => RenderItem::new(
                                    crate::game_logic::ObjectId(object_id),
                                    model_name.clone(),
                                    0,
                                    world_position,
                                    world_matrix,
                                    parent_material,
                                    render_pass,
                                ),
                                None => RenderItem::new_unbound_client_drawable(
                                    submission.drawable_id.0,
                                    model_name.clone(),
                                    0,
                                    world_position,
                                    world_matrix,
                                    parent_material,
                                    render_pass,
                                ),
                            };
                            Self::attach_bridge_draw_metadata(
                                &mut aggregate_parent_item,
                                &submission,
                            );
                            if let Some(rgba) = bridge_house_color {
                                aggregate_parent_item.apply_house_color_livery_with(
                                    rgba,
                                    w3d_model
                                        .meshes
                                        .first()
                                        .map(|m| m.name.as_str())
                                        .unwrap_or(""),
                                );
                            }
                            aggregate_parent_item.set_fow_visibility(fow_vis);
                            if let Some(state) = objectless_shroud {
                                aggregate_parent_item.set_frozen_objectless_drawable_shroud(state);
                            }
                            self.render_items.extend(
                                super::hlod_aggregate_render::collect_cached_hlod_aggregate_render_items(
                                    graphics_system,
                                    &aggregate_parent_item,
                                    &aggregate_poses,
                                    camera_position,
                                ),
                            );
                        }

                        // A source HLOD can consist solely of AdditionalModels.
                        // C++ still builds and renders those child objects even
                        // when the selected parent level contributes no rigid
                        // mesh, so retain the valid aggregate source rather
                        // than replacing its independent children with a debug
                        // cube.  Missing child prototypes remain individually
                        // absent through the cache-only collector above.
                        if w3d_model.meshes.is_empty()
                            && !has_source_aggregate_attachments
                            && Self::missing_model_debug_cubes_enabled()
                            && !real_w3d_name_resolved(model_name)
                        {
                            if let Some(fallback_model) =
                                graphics_system.get_model_or_fallback("__fallback_cube__")
                            {
                                if !fallback_model.meshes.is_empty() {
                                    let mut item = match owner_object_id {
                                        Some(object_id) => RenderItem::new(
                                            crate::game_logic::ObjectId(object_id),
                                            "__fallback_cube__".to_string(),
                                            0,
                                            world_position,
                                            world_matrix,
                                            &fallback_model.meshes[0].material,
                                            render_pass,
                                        ),
                                        None => RenderItem::new_unbound_client_drawable(
                                            submission.drawable_id.0,
                                            "__fallback_cube__".to_string(),
                                            0,
                                            world_position,
                                            world_matrix,
                                            &fallback_model.meshes[0].material,
                                            render_pass,
                                        ),
                                    };
                                    item.distance = world_position.distance(camera_position);
                                    item.set_fow_visibility(fow_vis);
                                    if let Some(state) = objectless_shroud {
                                        item.set_frozen_objectless_drawable_shroud(state);
                                    }
                                    self.render_items.push(item);
                                }
                            }
                        }
                        bridge_items_added += 1;
                    }
                }
                RenderModelLoadResult::SkippedByBudget | RenderModelLoadResult::Failed => {
                    if Self::missing_model_debug_cubes_enabled()
                        && !real_w3d_name_resolved(model_name)
                    {
                        if let Some(fallback_model) =
                            graphics_system.get_model_or_fallback("__fallback_cube__")
                        {
                            if !fallback_model.meshes.is_empty() {
                                let mut item = match owner_object_id {
                                    Some(object_id) => RenderItem::new(
                                        crate::game_logic::ObjectId(object_id),
                                        "__fallback_cube__".to_string(),
                                        0,
                                        world_position,
                                        world_matrix,
                                        &fallback_model.meshes[0].material,
                                        render_pass,
                                    ),
                                    None => RenderItem::new_unbound_client_drawable(
                                        submission.drawable_id.0,
                                        "__fallback_cube__".to_string(),
                                        0,
                                        world_position,
                                        world_matrix,
                                        &fallback_model.meshes[0].material,
                                        render_pass,
                                    ),
                                };
                                item.distance = world_position.distance(camera_position);
                                item.set_fow_visibility(fow_vis);
                                if let Some(state) = objectless_shroud {
                                    item.set_frozen_objectless_drawable_shroud(state);
                                }
                                self.render_items.push(item);
                                bridge_items_added += 1;
                            }
                        }
                    }
                }
            }
        }

        if bridge_items_added > 0 && self.frame_number.is_multiple_of(300) {
            debug!(
                "RenderBridge drain: {} items from {} submissions",
                bridge_items_added, submissions_count
            );
        }
    }

    /// Resolve C++ `DrawableInfo::m_shroudStatusObjectID` for one standalone
    /// client Drawable from the immutable host presentation snapshot.
    ///
    /// W3DScene starts objectless drawables at `OBJECTSHROUD_CLEAR`; only a
    /// controller whose frozen status is `FOGGED` or worse changes that result
    /// to `OBJECTSHROUD_SHROUDED`. Missing controllers, clear/partial/invalid
    /// controllers, and a missing presentation frame all remain clear. This
    /// function intentionally never reads `ObjectVisibility` or alpha.
    #[cfg(feature = "game_client")]
    fn frozen_objectless_drawable_shroud_for_submission(
        presentation: Option<&crate::presentation_frame::PresentationFrame>,
        submission: &game_client::render_bridge::DrawSubmission,
    ) -> Option<FrozenObjectlessDrawableShroudRenderState> {
        if submission.owner_object_id.is_some() {
            return None;
        }

        let controller_object_id = submission
            .shroud_status_object_id
            .filter(|object_id| *object_id != 0)
            .map(crate::game_logic::ObjectId);

        let controller_status = controller_object_id.and_then(|controller_id| {
            let frame = presentation?;
            frame
                .objects
                .iter()
                .find(|object| object.id == controller_id)
                .or_else(|| {
                    frame
                        .direct_host_drawables
                        .iter()
                        .find(|direct| direct.object.id == controller_id && direct.resident)
                        .map(|direct| &direct.object)
                })
                .map(|object| object.drawable_shroud.raw_status.as_game_logic_status())
        });

        let final_status = objectless_drawable_scene_status(controller_status);

        Some(FrozenObjectlessDrawableShroudRenderState {
            drawable_id: submission.drawable_id.0,
            controller_object_id,
            controller_found: controller_status.is_some(),
            final_status,
            pushes_projected_shroud_pass: final_status
                != gamelogic::common::types::ObjectShroudStatus::Clear,
        })
    }

    #[cfg(feature = "game_client")]
    pub(super) fn mesh_uv_override_for_submission(
        submission: &game_client::render_bridge::DrawSubmission,
        mesh_name: &str,
    ) -> Option<Vec2> {
        let leaf_name = mesh_name.rsplit('.').next().unwrap_or(mesh_name);
        submission
            .mesh_uv_overrides
            .iter()
            .filter(|override_state| {
                leaf_name
                    .get(..override_state.mesh_name_prefix.len())
                    .is_some_and(|prefix| {
                        prefix.eq_ignore_ascii_case(&override_state.mesh_name_prefix)
                    })
            })
            .max_by_key(|override_state| override_state.mesh_name_prefix.len())
            .map(|override_state| Vec2::new(override_state.u_offset, override_state.v_offset))
    }

    /// Sort render items for optimal rendering - equivalent to C++ RenderPipeline::SortRenderItems()
    pub(super) fn sort_render_items(&mut self) {
        self.render_items.sort_by(Self::compare_render_items);
    }

    /// Execute water rendering pass - equivalent to C++ RenderPipeline::ExecuteWaterPass()
    ///
    /// Uses the GameClient `TerrainVisual` water mesh (`sync_global_water_plane` /
    /// tiled `bake_water_tiles_world`) honoring GlobalData `water_position_z` /
    /// extents. Not a no-op when that mesh exists.
    pub(super) fn execute_water_pass(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        graphics_system: &GraphicsSystem,
    ) -> Result<()> {
        self.current_pass = Some(RenderPass::WaterPass);
        #[cfg(feature = "game_client")]
        {
            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(terrain_visual) = guard.as_mut() {
                    terrain_visual
                        .ensure_global_water_plane(graphics_system.device())
                        .map_err(|e| anyhow::anyhow!("water plane sync failed: {e}"))?;
                }
            }
            let vp_w = self.tactical_viewport_width.max(1.0);
            let vp_h = (self.tactical_viewport_height * self.tactical_view_height_frac).max(1.0);
            self.forward_pass.enqueue_pre_scene_callback(move |frame| {
                let depth_view = frame.depth_view_arc();
                let color_view = frame.color_view_arc();
                let encoder = frame.encoder();
                // UTBWATERNULL (documented diagnostic, GENERALS_UTBWATERNULL=1):
                // skip the main water pass together with the terrain water
                // lanes to prove whether unexplained screen artifacts
                // originate in the water family.
                if std::env::var("GENERALS_UTBWATERNULL").as_deref() == Ok("1") {
                    return Ok(());
                }
                let terrain_visual_guard =
                    game_client::terrain::terrain_visual::get_terrain_visual().ok();
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("main water pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_view.as_ref(),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: depth_view.as_ref().map(|depth| {
                        wgpu::RenderPassDepthStencilAttachment {
                            view: depth.as_ref(),
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                render_pass.set_viewport(0.0, 0.0, vp_w, vp_h, 0.0, 1.0);
                render_pass.set_scissor_rect(0, 0, vp_w as u32, vp_h as u32);
                if let Some(terrain_guard) = terrain_visual_guard.as_ref() {
                    if let Some(terrain_visual) = terrain_guard.as_ref() {
                        terrain_visual.record_water_draws(&mut render_pass);
                    }
                }
                drop(render_pass);
                Ok(())
            });
        }
        let _ = graphics_system;
        Ok(())
    }

    /// Execute UI rendering pass - equivalent to C++ RenderPipeline::ExecuteUIPass()
    pub(super) fn execute_ui_pass(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        _graphics_system: &GraphicsSystem,
    ) -> Result<()> {
        self.current_pass = Some(RenderPass::UIPass);
        // UI pass implementation would go here
        Ok(())
    }

    /// Get current render pass
    pub fn current_pass(&self) -> Option<RenderPass> {
        self.current_pass
    }

    /// Get frame number
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }

    /// Set the current viewing player for FOW calculations
    pub fn set_current_player(&mut self, player_id: u32) {
        self.current_player_id = player_id;
        trace!("RenderPipeline: Set current player to {}", player_id);
    }

    /// Get the current viewing player
    pub fn get_current_player(&self) -> u32 {
        self.current_player_id
    }
}

/// True when the requested model is a real W3D key (not the diagnostic cube).
fn real_w3d_name_resolved(model_name: &str) -> bool {
    let t = model_name.trim();
    !t.is_empty() && t != crate::assets::mesh_asset_resolve::PLACEHOLDER_MODEL_KEY
}

/// C++ `MeshGeometryClass` sets `SKIN` only after a complete influence chunk.
/// Either the header geometry type or a retained influence array is enough to
/// require a real HTree palette instead of the renderer's identity pad.
fn mesh_declares_skin(mesh: &crate::assets::W3DMesh) -> bool {
    crate::assets::W3DModel::mesh_declares_skin(mesh)
}

fn stamp_skinned_hierarchy_bind_pose(item: &mut RenderItem, mesh: &crate::assets::W3DMesh) {
    if mesh_declares_skin(mesh) {
        item.bone_palette_source = RenderItemBonePaletteSource::HierarchyBindPose;
    }
}

#[cfg(test)]
mod w3d_live_path_tests {
    use super::*;

    #[test]
    fn w3d_hlod_visibility_has_no_mesh_suffix_authority() {
        let src = include_str!("pipeline_collect.rs");
        // Assemble legacy identifiers so this source-level regression does
        // not make its own negative assertion fail.
        let suffix_visibility_helper = ["hlod", "subobject", "visible"].join("_");
        let condition_clip_helper = ["animation", "index", "for", "model", "condition"].join("_");
        assert!(
            !src.contains(&format!("fn {suffix_visibility_helper}")),
            "HLOD child visibility must come from source bit channels, never a mesh suffix"
        );
        assert!(
            !src.contains(&format!("fn {condition_clip_helper}")),
            "selected Draw-state animation identity must replace combat-bit clip guesses"
        );
        assert!(src.contains("mesh_local_transform_and_visibility_for_binding"));
        assert!(
            src.contains("mesh_visible_for_authored_subobject_directives"),
            "the frozen selected Draw state must reach active mesh submission"
        );
        assert!(src.contains("cached_draw_animation_binding"));
        let companion_loader = ["load", "companion", "animation"].join("_");
        assert!(
            !src.contains(&companion_loader),
            "frozen-frame collection must only consume the prewarm cache"
        );
        assert!(src.contains("submission.animation_name.as_deref()"));
        assert!(src.contains("submission.animation_time"));
        assert!(
            src.contains("mesh_visible_for_authored_subobject_directives"),
            "GameClient bridge submissions must apply C++ W3DModelDraw Hide/Show state"
        );
        assert!(
            src.contains("new_unbound_client_drawable"),
            "standalone C++ GameClient drawables must reach the renderer without an ObjectID cast"
        );
        let identity_tuple_fallback = ["unwrap_or((mesh", ".transform, true))"].join("");
        let identity_mesh_fallback = ["unwrap_or(mesh", ".transform)"].join("");
        assert!(
            !src.contains(&identity_tuple_fallback),
            "failed HLOD binds must skip, not identity-draw"
        );
        assert!(
            !src.contains(&identity_mesh_fallback),
            "failed HLOD binds must skip, not identity-draw"
        );
        assert!(
            src.contains("stamp_skinned_hierarchy_bind_pose"),
            "SKIN meshes must stamp a resolved HTree palette source"
        );
    }

    #[test]
    fn skin_meshes_stamp_hierarchy_bind_pose_not_identity_pad() {
        // C++ `MeshClass::Render` (`mesh.cpp:746-771`) deforms SKIN through
        // `Container->Get_HTree()`. Collect must stamp HierarchyBindPose so
        // the renderer does not upload the identity 64-mat pad.
        let mut skin = crate::assets::W3DMesh::new("skin".to_string());
        skin.vertex_influences = Some(vec![ww3d_core::w3d_format::W3dVertInfStruct {
            bone_idx: 1,
            pad: [0; 6],
        }]);
        let mut item = RenderItem::new(
            crate::game_logic::ObjectId(1),
            "SkinModel".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &crate::assets::W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        stamp_skinned_hierarchy_bind_pose(&mut item, &skin);
        assert_eq!(
            item.bone_palette_source,
            RenderItemBonePaletteSource::HierarchyBindPose
        );

        let rigid = crate::assets::W3DMesh::new("rigid".to_string());
        let mut rigid_item = RenderItem::new(
            crate::game_logic::ObjectId(2),
            "RigidModel".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &crate::assets::W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        stamp_skinned_hierarchy_bind_pose(&mut rigid_item, &rigid);
        assert_ne!(
            rigid_item.bone_palette_source,
            RenderItemBonePaletteSource::HierarchyBindPose
        );
    }

    #[test]
    fn w3d_hlod_visibility_bridge_uses_frozen_animation_identity_and_fraction() {
        let mut model = crate::assets::W3DModel::new("bridge_probe".to_string());
        model.animations.push(crate::assets::W3dAnimation {
            name: "BridgeClip".to_string(),
            hierarchy_name: "BridgeHier".to_string(),
            num_frames: 10,
            frame_rate: 30,
            source_is_compressed: false,
            channels: Vec::new(),
            raw_visibility_channels: Vec::new(),
            unsupported_visibility_pivots: Vec::new(),
        });

        model.hierarchy = Some(crate::assets::W3dHierarchy {
            name: "BridgeHier".to_string(),
            pivots: Vec::new(),
            pivot_fixups: Vec::new(),
        });

        let (binding, frame) =
            RenderPipeline::bridge_draw_animation(&model, Some("bridgehier.bridgeclip"), 0.5);
        assert!(matches!(
            binding,
            Some(crate::assets::W3dAnimationBinding::Local { index: 0 })
        ));
        assert_eq!(
            frame, 4.5,
            "the bridge fraction maps to the selected clip's source frame range"
        );
        let (missing_binding, missing_frame) =
            RenderPipeline::bridge_draw_animation(&model, Some("different.clip"), 0.5);
        assert!(
            missing_binding.is_none() && missing_frame == 0.0,
            "an unresolved named clip must not substitute animation zero"
        );
    }

    #[test]
    fn bridge_hlod_aggregate_path_does_not_require_a_rigid_parent_mesh() {
        let src = include_str!("pipeline_collect.rs");
        let bridge_body = src
            .split("pub(super) fn drain_render_bridge_submissions")
            .nth(1)
            .expect("bridge collector must remain present");
        assert!(
            bridge_body.contains("aggregate_attachment_poses_for_binding_and_capture_controls"),
            "bridge AdditionalModels must sample the frozen HAnim/capture pose"
        );
        assert!(
            bridge_body.contains("collect_cached_hlod_aggregate_render_items"),
            "bridge AdditionalModels must use the strict cache-only collector"
        );
        assert!(
            bridge_body.contains("has_source_aggregate_attachments"),
            "a valid aggregate-only source must be distinguished from a truly empty model"
        );
        assert!(
            bridge_body.contains("&& !has_source_aggregate_attachments"),
            "the debug fallback must not replace a valid aggregate-only source"
        );
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn bridge_render_items_retain_exact_draw_module_weapon_topology() {
        let mut item = RenderItem::new(
            crate::game_logic::ObjectId(41),
            "BridgeTopology".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &crate::assets::W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        let source = gamelogic::helpers::ModelDrawSourceIdentity {
            runtime_draw_ordinal: 3,
            module_name: "W3DModelDraw".to_string(),
            module_tag: "Turret".to_string(),
            module_tag_name_key: 0x1234,
        };
        let mut bindings = gamelogic::helpers::ModelDrawWeaponBoneBindings::default();
        bindings.fire_fx[0] = "WeaponFireFXBone".to_string();
        bindings.recoil[0] = "WeaponRecoilBone".to_string();
        bindings.muzzle_flash[0] = "WeaponMuzzleFlash".to_string();
        bindings.launch[0] = "WeaponLaunchBone".to_string();

        let mut submission = game_client::render_bridge::DrawSubmission::default();
        submission.legacy_model_draw_source = Some(source.clone());
        submission.legacy_weapon_bone_bindings = Some(bindings.clone());
        RenderPipeline::attach_bridge_draw_metadata(&mut item, &submission);

        assert_eq!(item.legacy_model_draw_source, Some(source.clone()));
        assert_eq!(item.legacy_weapon_bone_bindings, Some(bindings.clone()));

        // Aggregate render objects remain children of this exact DrawModule;
        // the typed source metadata survives the child-item handoff without
        // enabling any recoil/event behavior.
        let mut child = RenderItem::new_unbound_client_drawable(
            99,
            "BridgeTopologyChild".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &crate::assets::W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        child.copy_frozen_presentation_visuals_from(&item);
        assert_eq!(child.legacy_model_draw_source, Some(source));
        assert_eq!(child.legacy_weapon_bone_bindings, Some(bindings));
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn bridge_applies_cpp_placement_ghost_opacity_and_illegal_tint() {
        // C++ InGameUI.cpp:77-78, 1466, 3041 — setDrawableOpacity(0.45) + colorTint red.
        let mut item = RenderItem::new_unbound_client_drawable(
            0x504C_4143,
            "AmericaBarracks".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &crate::assets::W3DMaterial::default(),
            RenderPass::ForwardTransparent,
        );
        let mut submission = game_client::render_bridge::DrawSubmission::default();
        submission.render_state.opacity = 0.45;
        submission.render_state.construction_tint = Some([1.0, 0.0, 0.0]);
        RenderPipeline::attach_bridge_draw_metadata(&mut item, &submission);
        assert!(
            (item.presentation_opacity - 0.45).abs() < 1e-5,
            "placement ghost must use C++ placementOpacity 0.45"
        );
        assert!(
            (item.fow_visibility.visibility_alpha - 1.0).abs() < 1e-5,
            "placement opacity is not FOW"
        );
        assert!(item.material.diffuse_color.x > item.material.diffuse_color.y);
        assert!(item.material.diffuse_color.y.abs() < 1e-5);
    }

    #[test]
    fn real_w3d_name_does_not_count_as_fallback_cube() {
        assert!(real_w3d_name_resolved("avdozer"));
        assert!(!real_w3d_name_resolved("__fallback_cube__"));
        assert!(!real_w3d_name_resolved(""));
        let src = include_str!("pipeline_collect.rs");
        assert!(src.contains("!real_w3d_name_resolved(model_name)"));
    }
}
