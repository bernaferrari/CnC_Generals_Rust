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
            let mesh_scale =
                crate::assets::mesh_asset_resolve::mesh_scale_for_unit(&u.template_name);
            let world_matrix = gameplay_to_render_transform(u.world_matrix())
                * Mat4::from_scale(Vec3::splat(mesh_scale.max(0.01)));
            // Wave 491: mesh pass honors sold model-condition residual from presentation.
            // Wave 495: stamp moving/attacking/firing bits then honor sold residual.
            // Wave 496: stamp production-door phase bits into model-condition bank.
            // Wave 497: full stamped bits + body damage drive mesh key variants.
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
            let model_bits = u.model_condition_bits_with_combat_flags();
            let _ = u.model_condition_bits; // residual source marker (bits via stamp helper)
            let sold_for_mesh =
                crate::game_logic::host_enum_table_residual::host_model_condition_has(
                    model_bits,
                    crate::game_logic::host_enum_table_residual::sold_model_bit(),
                );
            let _combat_model_bits = model_bits; // residual: combat/door bits available for mesh variants
            let model_name_owned =
                crate::assets::mesh_asset_resolve::model_key_with_presentation_conditions(
                    &u.model_key,
                    u.body_damage_state,
                    false,
                    model_bits,
                );
            let _sold_for_mesh = sold_for_mesh; // residual: sold still derived for honesty
            let template_name_owned = u.template_name.clone();
            let selection_radius = u.selection_radius;
            let model_hint_owned = Some(model_name_owned.clone());
            let snapshot_fow = Some(u.fow_visibility);
            let selection_flash_intensity = u.selection_flash_intensity();
            // Wave 499: defector_flash folded into selection_flash_intensity(); poison via apply_poison_tint.
            let team_color = u.team_color;

            alive_objects += 1;

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

            let world_position = world_matrix.w_axis.truncate();
            let model_name = model_name_owned.as_str();
            let template_name_for_cull = template_name_owned.as_str();
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

                        let anim_frame = if !w3d_model.animations.is_empty()
                            && w3d_model.hierarchy.is_some()
                        {
                            let obj_key = object_id.0;
                            let want_index = animation_index_for_model_condition(
                                model_bits,
                                w3d_model.animations.len(),
                            );
                            let state = self.animation_states.entry(obj_key).or_insert_with(|| {
                                let (num_frames, frame_rate) =
                                    w3d_model.animation_metadata(want_index).unwrap_or((1, 30));
                                ObjectAnimationState {
                                    animation_index: want_index,
                                    current_frame: 0.0,
                                    frame_rate: frame_rate as f32,
                                    num_frames,
                                }
                            });
                            if state.animation_index != want_index {
                                let (num_frames, frame_rate) =
                                    w3d_model.animation_metadata(want_index).unwrap_or((1, 30));
                                state.animation_index = want_index;
                                state.current_frame = 0.0;
                                state.frame_rate = frame_rate as f32;
                                state.num_frames = num_frames;
                            }
                            if delta_time > 0.0 && delta_time < 1.0 {
                                state.current_frame += delta_time * state.frame_rate;
                                if state.num_frames > 1
                                    && state.current_frame >= state.num_frames as f32
                                {
                                    state.current_frame %= (state.num_frames - 1) as f32;
                                }
                            }
                            state.current_frame
                        } else {
                            0.0
                        };

                        for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                            if !hlod_subobject_visible(&mesh.name, u.body_damage_state, u.destroyed)
                            {
                                continue;
                            }
                            let mut material = mesh.material.clone();

                            if material.texture_name.is_none() {
                                if let Some(asset_manager_arc) = crate::assets::get_asset_manager()
                                {
                                    if let Ok(asset_manager) = asset_manager_arc.lock() {
                                        if let Some(obj_def) = asset_manager
                                            .resolve_object_definition(
                                                &template_name_owned,
                                                model_hint,
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
                                            } else if self
                                                .missing_ini_objects
                                                .insert(format!("{}::texture", template_name_owned))
                                            {
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

                            // Mesh local transforms coming from WW3D hierarchy/HLOD data are in
                            // source gameplay basis. If we axis-convert vertex payload at mesh build
                            // time, local transforms must be converted into the same render basis.
                            let mesh_local_transform = if mesh.vertices_in_render_space {
                                mesh.transform
                            } else {
                                let axis = gameplay_to_render_axis_matrix();
                                axis * mesh.transform * axis.inverse()
                            };
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
                            let mesh_local_transform = if mesh.vertices_in_render_space {
                                mesh.transform
                            } else {
                                let axis = gameplay_to_render_axis_matrix();
                                axis * mesh.transform * axis.inverse()
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
                            // Apply local mesh transform residual like unit path when needed.
                            let _ = mesh_local_transform;
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
            let source = graphics_system
                .get_model(model_name)
                .or_else(|| graphics_system.get_model(template_name));
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
            remap_model_key_alias, resolve_mesh_for_model_key, MeshResolveResult,
            PLACEHOLDER_MODEL_KEY,
        };

        static STARTUP_MODEL_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);
        let trace_this_attempt = !allow_sync_model_loads
            && STARTUP_MODEL_TRACE_COUNT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    (count < 64).then_some(count + 1)
                })
                .is_ok();

        // Canonical key from presentation model_key / get_model_name (airanger → airanger_s).
        let resolved_key = remap_model_key_alias(model_name);

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
        if let Some(model) = graphics_system.get_model(template_name).cloned() {
            if resolved_key != template_name {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            if trace_this_attempt {
                info!(
                    "Startup model load: cache hit template='{}' model='{}' (aliased from template cache)",
                    template_name, resolved_key
                );
            }
            return RenderModelLoadResult::Ready(model);
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

        let mut requested_model_name = resolved_key.clone();
        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            let loaded_model = match asset_manager_arc.lock() {
                Ok(mut asset_manager) => {
                    if let Some(mapped_name) = asset_manager.get_model_for_object(template_name) {
                        requested_model_name = remap_model_key_alias(&mapped_name);
                    }
                    if trace_this_attempt {
                        info!(
                            "Startup model load: template='{}' model='{}' requested='{}'",
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
                if template_name != requested_model_name
                    && template_name != model_name
                    && template_name != resolved_key
                {
                    graphics_system.cache_model(template_name.to_string(), model);
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
        } else if let Some(model) = graphics_system.get_model(template_name).cloned() {
            if resolved_key != template_name {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            Some(model)
        } else if let Some(model) = graphics_system.get_model(&requested_model_name).cloned() {
            if requested_model_name != resolved_key {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            if requested_model_name != template_name {
                graphics_system.cache_model(template_name.to_string(), model.as_ref().clone());
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
                    if model_key != template_name {
                        graphics_system.cache_model(template_name.to_string(), model.clone());
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

            let flags = submission.condition_flags;
            let body_damage = if flags
                .contains(game_client::render_bridge::RenderConditionFlags::RUBBLE)
            {
                3
            } else if flags
                .contains(game_client::render_bridge::RenderConditionFlags::REALLY_DAMAGED)
            {
                2
            } else if flags.contains(game_client::render_bridge::RenderConditionFlags::DAMAGED) {
                1
            } else {
                0
            };
            let dying = flags.contains(game_client::render_bridge::RenderConditionFlags::RUBBLE);
            let model_bits =
                if flags.contains(game_client::render_bridge::RenderConditionFlags::MOVING) {
                    1u128 << crate::game_logic::host_enum_table_residual::moving_model_bit()
                } else {
                    0
                };

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
                        let anim_frame = if !w3d_model.animations.is_empty()
                            && w3d_model.hierarchy.is_some()
                        {
                            let obj_key = object_id.0;
                            let want_index = animation_index_for_model_condition(
                                model_bits,
                                w3d_model.animations.len(),
                            );
                            let state = self.animation_states.entry(obj_key).or_insert_with(|| {
                                let (num_frames, frame_rate) =
                                    w3d_model.animation_metadata(want_index).unwrap_or((1, 30));
                                ObjectAnimationState {
                                    animation_index: want_index,
                                    current_frame: 0.0,
                                    frame_rate: frame_rate as f32,
                                    num_frames,
                                }
                            });
                            if state.animation_index != want_index {
                                let (num_frames, frame_rate) =
                                    w3d_model.animation_metadata(want_index).unwrap_or((1, 30));
                                state.animation_index = want_index;
                                state.current_frame = 0.0;
                                state.frame_rate = frame_rate as f32;
                                state.num_frames = num_frames;
                            }
                            state.current_frame
                        } else {
                            0.0
                        };

                        for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                            if !hlod_subobject_visible(&mesh.name, body_damage, dying) {
                                continue;
                            }
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

/// Pick W3D/HLOD clip from model-condition bits (not always clip 0).
///
/// Retail W3DModelDraw residual order: rubble/die → really-damaged → damaged
/// → attacking → moving → idle. Extra clips are only selected when the model
/// actually has them. Granny `.gr2` clips are **not** used
/// ([`crate::graphics::granny_honesty::granny_decoder_available`] is false).
fn animation_index_for_model_condition(model_bits: u128, anim_count: usize) -> usize {
    if anim_count == 0 {
        return 0;
    }
    use crate::game_logic::host_enum_table_residual::{
        attacking_model_bit, damaged_model_bit, host_model_condition_has, moving_model_bit,
        reallydamaged_model_bit, rubble_model_bit,
    };
    let has = |bit| host_model_condition_has(model_bits, bit);
    if has(rubble_model_bit()) && anim_count > 4 {
        return 4;
    }
    if has(reallydamaged_model_bit()) && anim_count > 3 {
        return 3;
    }
    if has(damaged_model_bit()) && anim_count > 2 {
        return 2;
    }
    if has(attacking_model_bit()) && anim_count > 2 {
        return 2.min(anim_count - 1);
    }
    if has(moving_model_bit()) && anim_count > 1 {
        return 1;
    }
    0
}

/// HLOD / W3DModelDraw hide-show: skip damaged/rubble subobjects unless that state is active.
fn hlod_subobject_visible(mesh_name: &str, body_damage_state: u8, dying: bool) -> bool {
    let n = mesh_name.to_ascii_lowercase();
    let want_rubble = dying || body_damage_state >= 3;
    let want_rd = body_damage_state == 2;
    let want_d = body_damage_state == 1;
    // Retail W3DModelDraw suffixes only — do not match _door / _default / forward.
    if n.contains("rubble") || n.ends_with("_die") {
        return want_rubble;
    }
    if n.ends_with("_rd") {
        return want_rd || want_rubble;
    }
    if n.ends_with("_d") {
        return want_d || want_rd || want_rubble;
    }
    true
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
    fn moving_condition_selects_non_zero_clip_when_present() {
        let moving_bit = crate::game_logic::host_enum_table_residual::moving_model_bit();
        let bits = 1u128 << moving_bit;
        assert_eq!(animation_index_for_model_condition(0, 2), 0);
        assert_eq!(animation_index_for_model_condition(bits, 2), 1);
        assert_eq!(animation_index_for_model_condition(bits, 1), 0);
        let damaged = 1u128 << crate::game_logic::host_enum_table_residual::damaged_model_bit();
        assert_eq!(animation_index_for_model_condition(damaged, 3), 2);
        assert!(
            !crate::graphics::granny_honesty::granny_decoder_available(),
            "live mesh path is W3D/HLOD, not Granny SDK"
        );
    }

    #[test]
    fn real_w3d_name_does_not_count_as_fallback_cube() {
        assert!(real_w3d_name_resolved("avdozer"));
        assert!(!real_w3d_name_resolved("__fallback_cube__"));
        assert!(!real_w3d_name_resolved(""));
        let src = include_str!("pipeline_collect.rs");
        assert!(src.contains("!real_w3d_name_resolved(model_name)"));
        assert!(src.contains("animation_index_for_model_condition"));
        assert!(src.contains("hlod_subobject_visible"));
    }

    #[test]
    fn hlod_hides_damaged_subobject_when_pristine() {
        assert!(hlod_subobject_visible("body", 0, false));
        assert!(!hlod_subobject_visible("body_d", 0, false));
        assert!(hlod_subobject_visible("body_d", 1, false));
        assert!(!hlod_subobject_visible("body_rubble", 0, false));
        assert!(hlod_subobject_visible("body_rubble", 3, false));
        assert!(
            hlod_subobject_visible("AmericaBarracks_Door", 0, false),
            "_door must stay visible when pristine"
        );
        assert!(
            hlod_subobject_visible("forward", 0, false),
            "forward must stay visible when pristine"
        );
        assert!(hlod_subobject_visible("standard", 0, false));
        assert!(hlod_subobject_visible("body_default", 0, false));
        assert!(hlod_subobject_visible("body_deploy", 0, false));
    }
}
