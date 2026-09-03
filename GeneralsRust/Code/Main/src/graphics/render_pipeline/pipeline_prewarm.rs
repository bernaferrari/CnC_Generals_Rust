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
    /// Resolve exact external raw HAnim files from the frozen presentation
    /// frame before the mesh collector runs. Collection itself may only read
    /// the resulting cache: opening an archive there would make a snapshot's
    /// selected Draw state depend on timing and could stall the render pass.
    ///
    /// This deliberately handles only the one-entry animation case that the
    /// active collector can select deterministically. Random/multiple entries
    /// and unsupported playback modes retain bind pose rather than warming an
    /// arbitrary clip.
    pub(super) fn prewarm_frozen_draw_animation_bindings(&mut self) {
        let Some(presentation) = self.presentation_frame.as_ref() else {
            return;
        };

        let mut requests = Vec::<(String, String)>::new();
        let mut seen = HashSet::new();
        for input in presentation.unit_render_inputs() {
            for draw_model in input.draw_models {
                if draw_model.model_key.trim().is_empty()
                    || !matches!(
                        &draw_model.animation_mode,
                        crate::assets::AuthoredDrawAnimationMode::Manual
                            | crate::assets::AuthoredDrawAnimationMode::Loop
                            | crate::assets::AuthoredDrawAnimationMode::Once
                            | crate::assets::AuthoredDrawAnimationMode::LoopBackwards
                            | crate::assets::AuthoredDrawAnimationMode::OnceBackwards
                    )
                {
                    continue;
                }
                let [animation] = draw_model.animations.as_slice() else {
                    continue;
                };
                let identity = animation.name.trim();
                if identity.is_empty() || identity.eq_ignore_ascii_case("none") {
                    continue;
                }
                // This is only an in-frame de-duplication optimization. The
                // persistent AssetManager cache uses identity + hierarchy,
                // never this model-key string.
                let request_key = format!(
                    "{}\u{0}{}",
                    draw_model.model_key.to_ascii_lowercase(),
                    identity.to_ascii_lowercase()
                );
                if seen.insert(request_key) {
                    requests.push((draw_model.model_key, identity.to_string()));
                }
            }
        }

        if requests.is_empty() {
            return;
        }

        let Some(asset_manager_arc) = crate::assets::get_asset_manager() else {
            return;
        };
        let Ok(mut asset_manager) = asset_manager_arc.lock() else {
            warn!("W3D Draw companion prewarm skipped: asset manager mutex poisoned");
            return;
        };
        for (model_key, identity) in requests {
            let _ = asset_manager.prewarm_w3d_draw_animation_binding(&model_key, &identity);
        }
    }

    /// Populate the strict C++ render-object registry for source HLOD
    /// `AdditionalModels` and rigid HMODEL connections before collection
    /// begins. Collection itself uses only cache lookups: opening an archive
    /// while building RenderItems would make a frozen presentation frame
    /// timing-dependent and can stall WGPU.
    ///
    /// This runs only from the already-authorized synchronous prewarm lane.
    /// It starts from resident normal models, then follows only exact HLOD and
    /// rigid HMODEL prototype records to a bounded token count. Missing,
    /// malformed, and SKIN_NODE source children are remembered as attempted or
    /// absent; they never turn into a presentation alias or fallback cube.
    pub(super) fn prewarm_cached_hlod_aggregate_render_objects(
        &mut self,
        graphics_system: &GraphicsSystem,
    ) {
        const MAX_HLOD_AGGREGATE_PREWARM_TOKENS: usize = 96;

        let mut pending = VecDeque::new();
        for (_, model) in graphics_system.get_all_models() {
            pending
                .extend(super::hlod_aggregate_render::aggregate_prototype_names_for_prewarm(model));
        }
        if pending.is_empty() {
            return;
        }

        let Some(asset_manager_arc) = crate::assets::get_asset_manager() else {
            return;
        };
        let Ok(mut asset_manager) = asset_manager_arc.lock() else {
            warn!("HLOD aggregate prewarm skipped: asset manager mutex poisoned");
            return;
        };

        let mut attempted_now = 0usize;
        let mut resolved_now = 0usize;
        while let Some(full_name) = pending.pop_front() {
            if attempted_now >= MAX_HLOD_AGGREGATE_PREWARM_TOKENS {
                break;
            }
            let identity_key = full_name.to_ascii_lowercase();
            if full_name.is_empty() || !self.hlod_aggregate_prewarm_attempts.insert(identity_key) {
                continue;
            }
            attempted_now += 1;

            let Some(prototype) =
                asset_manager.resolve_w3d_render_object_prototype_blocking(&full_name)
            else {
                continue;
            };
            resolved_now += 1;

            let Some(source_model) =
                asset_manager.cached_w3d_render_object_source_model(&prototype)
            else {
                continue;
            };
            match prototype.kind() {
                // An independently created HLOD can contribute its exact
                // constructor-selected LOD children and `AdditionalModels`.
                // The registry token's immutable index is authoritative: a
                // source W3D may contain several independent HLOD chunks.
                crate::assets::W3dRenderObjectPrototypeKind::Hlod { hlod_index } => {
                    pending.extend(
                        super::hlod_aggregate_render::hlod_prototype_names_for_prewarm(
                            source_model,
                            hlod_index,
                        ),
                    );
                }
                crate::assets::W3dRenderObjectPrototypeKind::Hmodel { hmodel_index } => {
                    // This intentionally reuses the HMODEL renderer's exact
                    // default-root/named-HTree validation. It sees only
                    // NODE/COLLISION_NODE; a SKIN_NODE cannot be warmed into
                    // an inferred-palette render path.
                    pending.extend(
                        super::hlod_aggregate_render::hmodel_rigid_node_names_for_prewarm(
                            source_model,
                            hmodel_index,
                        ),
                    );
                }
                crate::assets::W3dRenderObjectPrototypeKind::Collection { collection_index } => {
                    if let Some(collection) = source_model.collections.get(collection_index) {
                        pending.extend(collection.object_names.iter().cloned());
                    }
                }
                crate::assets::W3dRenderObjectPrototypeKind::Aggregate { aggregate_index } => {
                    if let Some(aggregate) = source_model.aggregates.get(aggregate_index) {
                        if !aggregate.base_model_name.is_empty() {
                            pending.push_back(aggregate.base_model_name.clone());
                        }
                        pending.extend(
                            aggregate
                                .subobjects
                                .iter()
                                .map(|sub| sub.subobject_name.clone()),
                        );
                    }
                }
                crate::assets::W3dRenderObjectPrototypeKind::DistLod { dist_lod_index } => {
                    if let Some(dist_lod) = source_model.dist_lods.get(dist_lod_index) {
                        pending.extend(dist_lod.lods.iter().map(|lod| lod.render_obj_name.clone()));
                    }
                }
                crate::assets::W3dRenderObjectPrototypeKind::Mesh { .. }
                | crate::assets::W3dRenderObjectPrototypeKind::Emitter { .. }
                | crate::assets::W3dRenderObjectPrototypeKind::Dazzle { .. }
                | crate::assets::W3dRenderObjectPrototypeKind::Box { .. }
                | crate::assets::W3dRenderObjectPrototypeKind::Ring { .. }
                | crate::assets::W3dRenderObjectPrototypeKind::Sphere { .. }
                | crate::assets::W3dRenderObjectPrototypeKind::Null { .. } => {}
            }
        }

        if attempted_now > 0 {
            debug!(
                "HLOD aggregate prewarm: attempted={} resolved={} remaining={} cap={}",
                attempted_now,
                resolved_now,
                pending.len(),
                MAX_HLOD_AGGREGATE_PREWARM_TOKENS
            );
        }
    }

    pub(super) fn prewarm_startup_models(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        allow_sync_model_loads: bool,
    ) {
        // Presentation-only: map signature comes from frozen world_env.
        let Some(pres) = self.presentation_frame.as_ref() else {
            return;
        };
        let map_name = pres.world_env.map_name.clone();
        let signature = pres.world_env.prewarm_signature(pres.fow_shell_bypass);

        if self
            .last_startup_model_prewarm_signature
            .as_deref()
            .is_some_and(|prev| prev == signature)
        {
            return;
        }

        // Prefer frozen prewarm names from PresentationWorldEnv (capped list).
        // When a presentation frame is installed, never re-query live map metadata
        // (empty prewarm list is fail-closed: skip names rather than dual-read logic).
        let template_names: Vec<String> = self
            .presentation_frame
            .as_ref()
            .map(|p| p.world_env.prewarm_template_names.clone())
            .unwrap_or_default();

        let mut candidates: Vec<String> = Vec::new();
        let mut seen = HashSet::new();

        if !template_names.is_empty() {
            if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
                if let Ok(asset_manager) = asset_manager_arc.lock() {
                    for template_raw in &template_names {
                        let template = template_raw.trim();
                        if template.is_empty() {
                            continue;
                        }
                        if !Self::should_prewarm_startup_map_template(&asset_manager, template) {
                            continue;
                        }
                        let key = template.to_ascii_lowercase();
                        if seen.insert(key) {
                            candidates.push(template.to_string());
                        }
                    }
                } else {
                    warn!("Startup model prewarm skipped: asset manager mutex poisoned");
                }
            }
        }

        if candidates.is_empty() {
            if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
                if let Ok(asset_manager) = asset_manager_arc.lock() {
                    candidates.extend(
                        asset_manager
                            .get_common_cnc_units()
                            .into_iter()
                            .map(str::to_string),
                    );
                }
            }
        } else if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            if let Ok(asset_manager) = asset_manager_arc.lock() {
                for unit in asset_manager.get_common_cnc_units() {
                    if candidates.len() >= if allow_sync_model_loads { 48 } else { 12 } {
                        break;
                    }
                    let key = unit.to_ascii_lowercase();
                    if seen.insert(key) {
                        candidates.push(unit.to_string());
                    }
                }
            }
        }

        let prewarm_limit = if allow_sync_model_loads { 48 } else { 12 };
        candidates.truncate(prewarm_limit);
        if candidates.is_empty() {
            self.last_startup_model_prewarm_signature = Some(signature);
            return;
        }

        let mut cached_to_graphics = 0usize;
        let mut stats = ModelPrewarmStats::default();

        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            match asset_manager_arc.lock() {
                Ok(mut asset_manager) => {
                    stats = asset_manager.prewarm_object_models_blocking(candidates.iter());
                    for name in &candidates {
                        if let Some(model) = asset_manager.get_cached_model(name) {
                            let resolved_name = asset_manager
                                .get_model_for_object(name)
                                .unwrap_or_else(|| name.clone());
                            graphics_system.cache_model(resolved_name.clone(), model.clone());
                            if resolved_name != *name {
                                graphics_system.cache_model(name.clone(), model);
                            }
                            cached_to_graphics += 1;
                        }
                    }
                }
                Err(_) => {
                    warn!("Startup model prewarm skipped: asset manager mutex poisoned");
                }
            }
        }

        info!(
            "Startup model prewarm: map='{}' candidates={} requested={} cache_hits={} resolved={} missing={} graphics_cached={}",
            if map_name.is_empty() {
                "<unknown>"
            } else {
                &map_name
            },
            candidates.len(),
            stats.requested,
            stats.cache_hits,
            stats.resolved,
            stats.missing,
            cached_to_graphics
        );

        self.last_startup_model_prewarm_signature = Some(signature);
    }

    pub(super) fn maybe_load_heightmap_hint_after_first_present(
        &mut self,
        graphics_system: &GraphicsSystem,
    ) {
        if !self.pending_heightmap_hint_load || self.frame_number <= 1 {
            return;
        }

        let Some(world_bounds) = self
            .presentation_frame
            .as_ref()
            .map(|p| p.world_env.world_bounds_vec3())
        else {
            return;
        };
        match self.load_heightmap_from_hint(
            &graphics_system.device_arc(),
            &graphics_system.queue_arc(),
            Some(world_bounds),
        ) {
            Ok(()) => {
                self.pending_heightmap_hint_load = false;
            }
            Err(err) => {
                warn!("Deferred heightmap hint load failed: {}", err);
                self.pending_heightmap_hint_load = false;
            }
        }
    }

    #[cfg(feature = "game_client")]
    pub(super) fn update_and_enqueue_terrain_pass(
        &mut self,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
    ) -> Result<()> {
        // Documented diagnostic gate: GENERALS_DISC_NOTERRAIN=1 skips the terrain
        // pre-scene pass entirely; with no pre-scene callbacks the WW3D scene pass
        // clears color+depth itself (wgpu_main_renderer clear path), probing
        // whether terrain-written depth evicts unit fragments. Default (unset) is
        // C++ parity: terrain draws first into the shared depth buffer
        // (W3DSceneRender pass order, dx8wrapper per-frame ZFUNC LESSEQUAL).
        if std::env::var("GENERALS_DISC_NOTERRAIN").as_deref() == Ok("1") {
            return Ok(());
        }
        static LOGGED_ZERO_TERRAIN_CHUNKS: AtomicBool = AtomicBool::new(false);
        static LOGGED_NONZERO_TERRAIN_CHUNKS: AtomicBool = AtomicBool::new(false);

        let terrain_pass_started = Instant::now();
        if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain_visual) = guard.as_mut() {
                let client_view_matrix = Mat4::from_cols_array_2d(&view_matrix.to_cols_array_2d());
                let client_projection_matrix =
                    Mat4::from_cols_array_2d(&projection_matrix.to_cols_array_2d());
                let terrain_render_started = Instant::now();
                terrain_visual
                    .render(&client_view_matrix, &client_projection_matrix)
                    .map_err(|e| {
                        anyhow::anyhow!("terrain visual render state update failed: {}", e)
                    })?;
                let terrain_render_elapsed = terrain_render_started.elapsed();
                let terrain_update_started = Instant::now();
                terrain_visual
                    .update()
                    .map_err(|e| anyhow::anyhow!("terrain visual update failed: {}", e))?;
                let terrain_update_elapsed = terrain_update_started.elapsed();

                let chunk_count = terrain_visual.chunk_draw_count();
                let terrain_total_elapsed = terrain_pass_started.elapsed();
                if terrain_total_elapsed >= PROFILE_STEP_LOG_THRESHOLD
                    || terrain_render_elapsed >= PROFILE_STEP_LOG_THRESHOLD
                    || terrain_update_elapsed >= PROFILE_STEP_LOG_THRESHOLD
                {
                    debug!(
                        "TerrainVisual breakdown: total={:?} render={:?} update={:?} visible_chunks={} total_chunks={} pending_visible_chunks={}",
                        terrain_total_elapsed,
                        terrain_render_elapsed,
                        terrain_update_elapsed,
                        terrain_visual.debug_visible_chunk_count(),
                        terrain_visual.debug_total_chunk_count(),
                        terrain_visual.debug_pending_visible_chunk_count()
                    );
                }
                if chunk_count == 0 {
                    if !LOGGED_ZERO_TERRAIN_CHUNKS.swap(true, Ordering::Relaxed) {
                        warn!(
                            "Terrain visual updated but no visible chunks were selected for drawing"
                        );
                    }
                } else if !LOGGED_NONZERO_TERRAIN_CHUNKS.swap(true, Ordering::Relaxed) {
                    info!(
                        "Terrain visual selected {} visible chunks for drawing",
                        chunk_count
                    );
                }
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        }

        let _view = *view_matrix;
        let _projection = *projection_matrix;
        // Terrain is the first color pass; later scene/water Load the
        // backbuffer. Clear BLACK like C++ Begin_Render, not fog peach.
        let clear_color = self.terrain_clear_color();
        let vp_w = self.tactical_viewport_width.max(1.0);
        let vp_h = (self.tactical_viewport_height * self.tactical_view_height_frac).max(1.0);
        self.forward_pass.enqueue_pre_scene_callback(move |frame| {
            let terrain_draw_started = Instant::now();
            let depth_view = frame.depth_view_arc();
            let color_view = frame.color_view_arc();
            let encoder = frame.encoder();
            let terrain_visual_guard =
                game_client::terrain::terrain_visual::get_terrain_visual().ok();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main terrain pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.as_ref().map(|depth| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: depth.as_ref(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
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
                    terrain_visual.record_chunk_draws(&mut render_pass);
                }
            }
            drop(render_pass);

            let terrain_draw_elapsed = terrain_draw_started.elapsed();
            if terrain_draw_elapsed >= PROFILE_STEP_LOG_THRESHOLD {
                debug!(
                    "TerrainVisual chunk draw recording took {:?}",
                    terrain_draw_elapsed
                );
            }

            Ok(())
        });

        Ok(())
    }
}
