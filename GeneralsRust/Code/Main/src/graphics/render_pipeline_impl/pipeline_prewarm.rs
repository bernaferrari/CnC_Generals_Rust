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
            if map_name.is_empty() { "<unknown>" } else { &map_name },
            candidates.len(),
            stats.requested,
            stats.cache_hits,
            stats.resolved,
            stats.missing,
            cached_to_graphics
        );

        self.last_startup_model_prewarm_signature = Some(signature);
    }

    pub(super) fn maybe_load_heightmap_hint_after_first_present(&mut self, graphics_system: &GraphicsSystem) {
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
                        warn!("Terrain visual updated but no visible chunks were selected for drawing");
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
        let clear_color = self.terrain_clear_color();
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
