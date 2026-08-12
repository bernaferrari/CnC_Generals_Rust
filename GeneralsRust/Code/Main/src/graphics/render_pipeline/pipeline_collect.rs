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
    /// Advance one exact source-selected W3DModelDraw state. The C++ draw
    /// module chooses its first entry only when it has a single animation;
    /// random idle/multiple-entry selection has not been ported, so that case
    /// retains the entries but deliberately remains bind-pose rather than
    /// guessing a client-side random clip.
    fn advance_authored_draw_animation(
        &mut self,
        object_id: crate::game_logic::ObjectId,
        draw_module_index: u32,
        model: &crate::assets::W3DModel,
        authored_animations: &[crate::assets::AuthoredDrawAnimation],
        authored_mode: &crate::assets::AuthoredDrawAnimationMode,
        delta_time: f32,
    ) -> (Option<usize>, f32) {
        let requested_index = match authored_animations {
            [] => Some(None),
            [animation] => model
                .find_animation_index_for_draw_identity(&animation.name)
                .map(Some),
            // C++ uses GameClientRandomValue for multiple state animations.
            // Until that exact stateful selector is ported, fail closed to
            // bind pose rather than choosing an arbitrary W3D index.
            _ => None,
        };
        let Some(requested_index) = requested_index else {
            return (None, 0.0);
        };
        let mode_is_supported = matches!(
            authored_mode,
            crate::assets::AuthoredDrawAnimationMode::Manual
                | crate::assets::AuthoredDrawAnimationMode::Loop
                | crate::assets::AuthoredDrawAnimationMode::Once
                | crate::assets::AuthoredDrawAnimationMode::LoopBackwards
                | crate::assets::AuthoredDrawAnimationMode::OnceBackwards
        );
        if !mode_is_supported {
            return (None, 0.0);
        }

        let Some(animation_index) = requested_index else {
            return (None, 0.0);
        };
        if model.hierarchy.is_none() {
            return (None, 0.0);
        }

        let (num_frames, frame_rate) = model.animation_metadata(animation_index).unwrap_or((1, 30));
        let obj_key = (object_id.0, draw_module_index);
        let state = self.animation_states.entry(obj_key).or_insert_with(|| {
            let start_frame = match authored_mode {
                crate::assets::AuthoredDrawAnimationMode::LoopBackwards
                | crate::assets::AuthoredDrawAnimationMode::OnceBackwards => {
                    num_frames.saturating_sub(1) as f32
                }
                _ => 0.0,
            };
            ObjectAnimationState {
                animation_index: Some(animation_index),
                current_frame: start_frame,
                frame_rate: frame_rate as f32,
                num_frames,
                mode: authored_mode.clone(),
            }
        });
        if state.animation_index != Some(animation_index) || state.mode != authored_mode.clone() {
            state.animation_index = Some(animation_index);
            state.current_frame = match authored_mode {
                crate::assets::AuthoredDrawAnimationMode::LoopBackwards
                | crate::assets::AuthoredDrawAnimationMode::OnceBackwards => {
                    num_frames.saturating_sub(1) as f32
                }
                _ => 0.0,
            };
            state.frame_rate = frame_rate as f32;
            state.num_frames = num_frames;
            state.mode = authored_mode.clone();
        }

        if delta_time > 0.0 && delta_time < 1.0 && state.num_frames > 1 {
            let terminal = (state.num_frames - 1) as f32;
            let delta = delta_time * state.frame_rate;
            state.current_frame = match &state.mode {
                crate::assets::AuthoredDrawAnimationMode::Manual => state.current_frame,
                crate::assets::AuthoredDrawAnimationMode::Once => {
                    (state.current_frame + delta).min(terminal)
                }
                crate::assets::AuthoredDrawAnimationMode::Loop => {
                    let period = terminal;
                    if period > 0.0 {
                        (state.current_frame + delta) % period
                    } else {
                        0.0
                    }
                }
                crate::assets::AuthoredDrawAnimationMode::OnceBackwards => {
                    (state.current_frame - delta).max(0.0)
                }
                crate::assets::AuthoredDrawAnimationMode::LoopBackwards => {
                    let period = terminal;
                    if period > 0.0 {
                        (state.current_frame - delta).rem_euclid(period)
                    } else {
                        0.0
                    }
                }
                crate::assets::AuthoredDrawAnimationMode::LoopPingPong => {
                    // This mode requires a direction bit in the state. Keep
                    // the current source frame stable until that complete
                    // playback state is ported instead of pretending it loops.
                    state.current_frame
                }
                crate::assets::AuthoredDrawAnimationMode::Unsupported(_) => state.current_frame,
            };
        }
        (Some(animation_index), state.current_frame)
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
    ) -> (Option<usize>, f32) {
        let Some(animation_name) = animation_name else {
            return (None, 0.0);
        };
        let Some(animation_index) = model.find_animation_index_for_draw_identity(animation_name)
        else {
            return (None, 0.0);
        };
        let Some((num_frames, _)) = model.animation_metadata(animation_index) else {
            return (None, 0.0);
        };
        if !animation_time.is_finite() {
            return (None, 0.0);
        }
        let frame = animation_time.clamp(0.0, 1.0) * num_frames.saturating_sub(1) as f32;
        (Some(animation_index), frame)
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
    ) -> Result<()> {
        let collect_started = Instant::now();
        let object_ids_started = Instant::now();
        // Snapshot ownership: when presentation is present, drive the main unit
        // mesh pass from unit_render_inputs (no live object identity / FOW re-read).
        // Keep frame installed for post-collect execute residual (minimap/shell/heightmap).
        let presentation = self.presentation_frame.clone();
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
            // Wave 512: CONTINUOUS_FIRE / PRONE / PREATTACK / TURRET_ROTATE bits included in stamp helper.
            // Wave 513: JAMMED / DYING / RELOADING / PACKING / UNPACKING bits included in stamp helper.
            // Wave 515: RAISING_FLAG (surrendered) bit included in stamp helper.
            let template_name_owned = u.template_name.clone();
            let selection_radius = u.selection_radius;
            let snapshot_fow = Some(u.fow_visibility);
            let selection_flash_intensity = u.selection_flash_intensity();
            // Wave 499: defector_flash folded into selection_flash_intensity(); poison via apply_poison_tint.
            let team_color = u.team_color;
            // `UnitRenderInput::from_renderable` normalizes old snapshots to
            // one module. Keep the same compatibility at this boundary for
            // direct test/boot inputs which still provide only `model_key`.
            let draw_models = if u.draw_models.is_empty() {
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
                        object_id,
                        self.current_player_id
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
                let authored_animations = draw_model.animations;
                let authored_animation_mode = draw_model.animation_mode;
                let model_name_owned = draw_model.model_key;
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
                        if w3d_model.meshes.is_empty() {
                            self.debug_last_zero_mesh_models += 1;
                            // Fall through to fallback cube below (same as Failed path)
                        } else {
                            let visibility = fow_visibility;

                            let (animation_index, anim_frame) = self
                                .advance_authored_draw_animation(
                                    object_id,
                                    draw_module_index,
                                    &w3d_model,
                                    &authored_animations,
                                    &authored_animation_mode,
                                    delta_time,
                                );

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
                                                template_name_owned,
                                                model_hint
                                            );
                                            }
                                        }
                                    }
                                }

                                // HLOD rigid children resolve through the source-authored
                                // `HLOD.Name.MeshName -> BoneIndex` record.  Unsupported
                                // multi-LOD/aggregate content returns None and remains
                                // intentionally non-rendering rather than drawing every group.
                                let Some((mesh_local_transform, mesh_visible)) = w3d_model
                                    .mesh_local_transform_and_visibility_for_animation(
                                        mesh_idx,
                                        animation_index,
                                        anim_frame,
                                    )
                                else {
                                    continue;
                                };
                                if !mesh_visible {
                                    continue;
                                }
                                let mesh_local_transform = if transform_is_reasonable_for_mesh(
                                    mesh_local_transform,
                                ) {
                                    mesh_local_transform
                                } else {
                                    let key = format!(
                                        "{}::{}::{}",
                                        template_name_owned, model_name, mesh.name
                                    );
                                    if self.debug_warned_bad_mesh_transforms.insert(key.clone()) {
                                        warn!(
                                        "Invalid mesh local transform for '{}': template='{}' model='{}' mesh='{}'; using identity transform",
                                        key, template_name_owned, model_name, mesh.name
                                    );
                                    }
                                    Mat4::IDENTITY
                                };
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
                                render_item.distance = world_position.distance(camera_position);
                                render_item.set_fow_visibility(visibility);
                                render_item.animation_frame = anim_frame;

                                self.render_items.push(render_item);
                            }

                            trace!(
                                "Object {} will render with FOW alpha={}, explored={}",
                                object_id,
                                visibility.visibility_alpha,
                                visibility.is_explored
                            );
                            render_item_build_elapsed += render_item_build_started.elapsed();
                            continue; // Skip the fallback path
                        }

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
                                    render_item.set_fow_visibility(fow_visibility);
                                    if selection_flash_intensity > 0.0 {
                                        render_item.apply_selection_flash(
                                            selection_flash_intensity,
                                            team_color,
                                        );
                                    }
                                    // Wave 499: presentation poison tint residual (no live GameLogic).
                                    if u.poison_tinted {
                                        render_item.apply_poison_tint();
                                    }

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
                                    render_item.set_fow_visibility(fow_visibility);
                                    if selection_flash_intensity > 0.0 {
                                        render_item.apply_selection_flash(
                                            selection_flash_intensity,
                                            team_color,
                                        );
                                    }
                                    // Wave 499: presentation poison tint residual (no live GameLogic).
                                    if u.poison_tinted {
                                        render_item.apply_poison_tint();
                                    }

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
                            let Some(mesh_local_transform) =
                                w3d_model.mesh_local_transform_for_animation(mesh_idx, 0, 0.0)
                            else {
                                continue;
                            };
                            let mesh_local_transform =
                                if transform_is_reasonable_for_mesh(mesh_local_transform) {
                                    mesh_local_transform
                                } else {
                                    Mat4::IDENTITY
                                };
                            let mut render_item = RenderItem::new(
                                p.id,
                                model_name.to_string(),
                                mesh_idx,
                                world_position,
                                world_matrix,
                                &material,
                                Self::render_pass_for_material(&material),
                            );
                            render_item.set_mesh_local_transform(mesh_local_transform);
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

        let world_scale = world_matrix
            .x_axis
            .truncate()
            .length()
            .max(world_matrix.y_axis.truncate().length())
            .max(world_matrix.z_axis.truncate().length())
            .max(1.0);
        // Gameplay/render space is X/Z ground, Y-up (gameplay_to_render_transform is
        // identity). Use a building-sized fallback: selection_radius 0→10 is far too
        // small for CCs/factories and culls them before the mesh is ever loaded.
        let fallback_radius = selection_radius.max(Self::structure_cull_fallback_radius(
            template_name,
            model_name,
        ));
        let fallback_center = world_matrix.w_axis.truncate();
        model_bounds
            .map(|(local_center, local_radius)| {
                let world_center = world_matrix.transform_point3(local_center);
                let world_radius = (local_radius * world_scale).max(fallback_radius);
                (world_center, world_radius)
            })
            .unwrap_or((fallback_center, fallback_radius))
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
        let min = model.bounding_box_min;
        let max = model.bounding_box_max;
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
            canonical_model_key, resolve_mesh_for_model_key, MeshResolveResult,
            PLACEHOLDER_MODEL_KEY,
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
    pub(super) fn drain_render_bridge_submissions(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        camera_position: Vec3,
        deferred_model_load_budget: &mut usize,
    ) {
        use game_client::render_bridge::get_render_bridge;

        let mut bridge_guard = match get_render_bridge().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let bridge = match bridge_guard.as_mut() {
            Some(b) => b,
            None => return,
        };

        bridge.flush();

        let submissions = bridge.drain_scene_submissions();
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

            let object_id = crate::game_logic::ObjectId(submission.drawable_id.0);
            let vis_alpha = submission.render_state.opacity;
            let fow_vis = ObjectVisibility {
                visibility_alpha: vis_alpha,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            };

            let load_result = Self::ensure_render_model_loaded(
                graphics_system,
                model_name,
                model_name,
                true,
                deferred_model_load_budget,
            );

            match load_result {
                RenderModelLoadResult::Ready(w3d_model) => {
                    if w3d_model.meshes.is_empty() {
                        // Real W3D name resolved — do not plant a diagnostic cube.
                        if Self::missing_model_debug_cubes_enabled()
                            && !real_w3d_name_resolved(model_name)
                        {
                            if let Some(fallback_model) =
                                graphics_system.get_model_or_fallback("__fallback_cube__")
                            {
                                if !fallback_model.meshes.is_empty() {
                                    let mut item = RenderItem::new(
                                        object_id,
                                        "__fallback_cube__".to_string(),
                                        0,
                                        world_position,
                                        world_matrix,
                                        &fallback_model.meshes[0].material,
                                        render_pass,
                                    );
                                    item.distance = world_position.distance(camera_position);
                                    item.set_fow_visibility(fow_vis);
                                    self.render_items.push(item);
                                    bridge_items_added += 1;
                                }
                            }
                        }
                    } else {
                        // GameClient freezes its selected W3DModelDraw clip
                        // and normalized frame fraction into the bridge
                        // submission. Apply that exact state so raw W3D bit
                        // channels govern HLOD children here too.
                        let (animation_index, anim_frame) = Self::bridge_draw_animation(
                            &w3d_model,
                            submission.animation_name.as_deref(),
                            submission.animation_time,
                        );

                        for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                            let Some((mesh_local_transform, mesh_visible)) = w3d_model
                                .mesh_local_transform_and_visibility_for_animation(
                                    mesh_idx,
                                    animation_index,
                                    anim_frame,
                                )
                            else {
                                continue;
                            };
                            if !mesh_visible {
                                continue;
                            }
                            let mesh_local_transform =
                                if transform_is_reasonable_for_mesh(mesh_local_transform) {
                                    mesh_local_transform
                                } else {
                                    Mat4::IDENTITY
                                };
                            let mut item = RenderItem::new(
                                object_id,
                                model_name.clone(),
                                mesh_idx,
                                world_position,
                                world_matrix,
                                &mesh.material,
                                render_pass,
                            );
                            item.distance = world_position.distance(camera_position);
                            item.set_fow_visibility(fow_vis);
                            item.animation_frame = anim_frame;
                            item.set_mesh_local_transform(mesh_local_transform);
                            item.uv_offset_override =
                                Self::mesh_uv_override_for_submission(&submission, &mesh.name);
                            self.render_items.push(item);
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
                                let mut item = RenderItem::new(
                                    object_id,
                                    "__fallback_cube__".to_string(),
                                    0,
                                    world_position,
                                    world_matrix,
                                    &fallback_model.meshes[0].material,
                                    render_pass,
                                );
                                item.distance = world_position.distance(camera_position);
                                item.set_fow_visibility(fow_vis);
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
            // Draw the existing tiled water mesh (Load so we do not clear terrain).
            self.forward_pass.enqueue_pre_scene_callback(move |frame| {
                let depth_view = frame.depth_view_arc();
                let color_view = frame.color_view_arc();
                let encoder = frame.encoder();
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
        assert!(src.contains("mesh_local_transform_and_visibility_for_animation"));
        assert!(src.contains("submission.animation_name.as_deref()"));
        assert!(src.contains("submission.animation_time"));
    }

    #[test]
    fn w3d_hlod_visibility_bridge_uses_frozen_animation_identity_and_fraction() {
        let mut model = crate::assets::W3DModel::new("bridge_probe".to_string());
        model.animations.push(crate::assets::W3dAnimation {
            name: "BridgeClip".to_string(),
            hierarchy_name: "BridgeHier".to_string(),
            num_frames: 10,
            frame_rate: 30,
            channels: Vec::new(),
            raw_visibility_channels: Vec::new(),
            unsupported_visibility_pivots: Vec::new(),
        });

        assert_eq!(
            RenderPipeline::bridge_draw_animation(&model, Some("bridgehier.bridgeclip"), 0.5,),
            (Some(0), 4.5),
            "the bridge fraction maps to the selected clip's source frame range"
        );
        assert_eq!(
            RenderPipeline::bridge_draw_animation(&model, Some("different.clip"), 0.5),
            (None, 0.0),
            "an unresolved named clip must not substitute animation zero"
        );
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
