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

impl ForwardPass {
    pub(super) fn initialize() -> Result<Self> {
        // Initialize WW3D renderer - this may fail if engine is not initialized
        let clear_color = if std::env::var_os("GENERALS_DEBUG_WW3D_CLEAR_COLOR").is_some() {
            Vec4::new(0.0, 0.55, 0.0, 1.0)
        } else {
            Vec4::new(0.0, 0.0, 0.0, 1.0)
        };
        let renderer_config = WgpuMainRendererConfig {
            clear_color,
            ..WgpuMainRendererConfig::default()
        };
        let renderer = WgpuMainRenderer::from_engine(renderer_config)
            .map_err(|e| anyhow::anyhow!("Failed to initialize WW3D renderer: {e:?}"))?;

        // Get engine device and queue - these are Arc clones of the global engine resources
        let device =
            ww3d_engine::device().map_err(|e| anyhow::anyhow!("WW3D device unavailable: {e:?}"))?;
        let queue =
            ww3d_engine::queue().map_err(|e| anyhow::anyhow!("WW3D queue unavailable: {e:?}"))?;

        info!("ForwardPass initialized successfully");

        Ok(Self {
            renderer,
            mesh_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            pending_texture_stream: VecDeque::new(),
            queued_texture_stream: HashSet::new(),
            fallback_texture: None,
            camera: CameraClass::new(),
            device,
            queue,
        })
    }

    /// Check if the forward pass is ready to render
    /// Returns true if all required resources are available
    pub(super) fn is_ready(&self) -> bool {
        // Verify engine is still initialized by checking if we can get device/queue
        // The Arc references we hold should still be valid, but engine might have shut down
        ww3d_engine::device().is_ok() && ww3d_engine::queue().is_ok()
    }

    #[allow(unused_assignments)]
    pub(super) fn prewarm_textures_blocking<I, S>(&mut self, texture_names: I) -> Result<TexturePrewarmStats>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut stats = TexturePrewarmStats::default();
        let mut unique = HashSet::new();
        let mut requested: Vec<(String, String)> = Vec::new();

        for texture_name in texture_names {
            let texture_name = texture_name.as_ref().trim();
            if !Self::is_valid_texture_name(texture_name) {
                continue;
            }
            let cache_key = texture_name.to_ascii_lowercase();
            if !unique.insert(cache_key.clone()) {
                continue;
            }
            stats.requested += 1;
            if self.texture_cache.contains_key(&cache_key) {
                stats.cache_hits += 1;
                continue;
            }
            requested.push((texture_name.to_string(), cache_key));
        }

        // Prime all raw payloads while holding the asset manager lock once, matching C++ upfront loads.
        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("Asset manager mutex poisoned"))?;
            asset_manager.prime_textures_raw_blocking(requested.iter().map(|(name, _)| name));
        }

        for (texture_name, cache_key) in requested {
            if self.texture_cache.contains_key(&cache_key) {
                stats.cache_hits += 1;
                continue;
            }

            if self.is_known_missing_texture(&texture_name) {
                stats.missing += 1;
                if let Ok(fallback) = self.ensure_fallback_texture() {
                    self.texture_cache.insert(cache_key, fallback);
                }
                continue;
            }

            if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                self.texture_cache.insert(cache_key, texture);
                stats.resolved += 1;
            } else {
                let _ = self.prime_texture_raw_blocking(&texture_name);
                if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                    self.texture_cache.insert(cache_key, texture);
                    stats.resolved += 1;
                } else {
                    self.queue_texture_stream(&texture_name);
                }
            }
        }

        // Drain queued texture stream before first visible menu frame.
        for _ in 0..32 {
            if self.pending_texture_stream.is_empty() {
                break;
            }

            let pending_before = self.pending_texture_stream.len();
            let budget = pending_before.clamp(64, 2048);
            self.stream_pending_textures(budget);
            if self.pending_texture_stream.len() >= pending_before {
                break;
            }
        }
        stats.queued_remaining = self.pending_texture_stream.len();
        Ok(stats)
    }

    #[allow(unused_assignments)]
    pub(super) fn render(
        &mut self,
        graphics_system: &GraphicsSystem,
        render_items: &[RenderItem],
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        lighting: Option<&CachedLighting>,
    ) -> Result<()> {
        // Check if renderer is ready before attempting to render
        // This prevents crashes when engine is shutting down or not initialized
        if !self.is_ready() {
            warn!("ForwardPass::render - engine not ready, skipping frame");
            return Ok(());
        }

        // C++ parity: first-use textures should resolve quickly; avoid one-texture-per-frame trickle.
        self.stream_pending_textures(self.texture_stream_budget());

        static FP_FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let fp_frame = FP_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if fp_frame < 5 {
            info!("ForwardPass::render #{} begin_frame_start", fp_frame);
        }

        // Begin frame - initialize render state
        self.renderer
            .begin_frame()
            .map_err(|e| anyhow::anyhow!("WW3D renderer begin_frame failed: {e:?}"))?;

        if fp_frame < 5 {
            info!("ForwardPass::render #{} begin_frame_done", fp_frame);
        }

        let mut queued_count_total = 0usize;
        let mut queue_error_total = 0usize;

        // Scope to ensure mutex lock is released before end_frame
        {
            let renderer_handle = self.renderer.renderer_handle();

            // Attempt to lock renderer - handle both poisoned and unavailable cases
            let mut renderer = match renderer_handle.try_lock() {
                Ok(guard) => guard,
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(anyhow::anyhow!("WW3D renderer handle poisoned - another thread panicked while holding the lock"));
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    warn!("WW3D renderer handle already locked - skipping frame");
                    // Still need to end_frame to maintain state
                    self.renderer.end_frame().map_err(|e| {
                        anyhow::anyhow!(
                            "WW3D renderer end_frame failed after lock contention: {e:?}"
                        )
                    })?;
                    return Ok(());
                }
            };

            // Update camera state - must happen before queueing meshes
            self.camera.set_view_matrix(*view_matrix);
            self.camera.set_projection_matrix(*projection_matrix);
            self.camera.set_position(camera_position);
            renderer.set_camera(self.camera.clone());
            renderer.set_light_environment(Self::build_light_environment(lighting));
            Self::log_visibility_probe(
                render_items,
                view_matrix,
                projection_matrix,
                camera_position,
            );
            Self::log_material_probe(render_items);

            if render_items.is_empty() {
                trace!("ForwardPass::render - presenting empty scene frame");
            }

            // Queue opaque + transparent geometry for rendering
            let mut queued_count = 0;
            let mut error_count = 0;
            let mut hidden_count = 0usize;
            let renderable_passes = [RenderPass::ForwardOpaque, RenderPass::ForwardTransparent];

            for item in render_items {
                if !renderable_passes.contains(&item.render_pass) {
                    continue;
                }
                if item.fow_visibility.visibility_alpha <= 0.01 {
                    hidden_count += 1;
                    continue;
                }

                // Prepare mesh instance - handles missing models gracefully
                match self.prepare_mesh_instance(graphics_system, item) {
                    Ok(Some(mesh)) => {
                        // Queue mesh for rendering
                        if let Err(e) = renderer.queue_mesh(mesh) {
                            error!("Failed to queue mesh for item {}: {e:?}", item.object_id);
                            error_count += 1;
                            // Continue processing other items instead of failing entire frame
                            continue;
                        }
                        queued_count += 1;
                    }
                    Ok(None) => {
                        // Model not available - already logged in prepare_mesh_instance
                        continue;
                    }
                    Err(e) => {
                        error!("Failed to prepare mesh for item {}: {e}", item.object_id);
                        error_count += 1;
                        // Continue processing other items
                        continue;
                    }
                }
            }

            trace!(
                "ForwardPass::render - queued {}/{} opaque+transparent items ({} errors, {} hidden-by-alpha)",
                queued_count,
                render_items.len(),
                error_count,
                hidden_count
            );
            queued_count_total = queued_count;
            queue_error_total = error_count;
        } // Mutex lock released here

        // C++ parity: after 3D scene, flush the 2D UI overlay (Shell menus,
        // WindowManager windows) on top of the rendered scene. This is the
        // post-scene 2D pass where gadget draw callbacks render.
        self.renderer.enqueue_post_frame_callback(|frame| {
            crate::graphics::ui_render_pass::flush_ui_to_frame(frame)
        });

        // End frame - submit queued work to GPU (runs post-frame callbacks first)
        self.renderer
            .end_frame()
            .map_err(|e| anyhow::anyhow!("WW3D renderer end_frame failed: {e:?}"))?;

        let stats = self.renderer.stats();
        if queued_count_total > 0 {
            debug!(
                "ForwardPass presented: queued={} queue_errors={} draw_calls={} meshes={} tris={}",
                queued_count_total,
                queue_error_total,
                stats.draw_calls,
                stats.meshes_rendered,
                stats.triangles_rendered
            );
        }

        Ok(())
    }

    pub(super) fn build_light_environment(lighting: Option<&CachedLighting>) -> Option<LightEnvironmentClass> {
        let mut env = LightEnvironmentClass::new();
        let have_metadata = lighting
            .map(|v| {
                v.sun_direction.is_some()
                    || v.sun_color.is_some()
                    || v.ambient_color.is_some()
                    || v.fog_color.is_some()
                    || v.fog_range.is_some()
            })
            .unwrap_or(false);

        let ambient = lighting
            .and_then(|v| v.ambient_color)
            .or_else(|| lighting.and_then(|v| v.fog_color))
            .or_else(|| lighting.and_then(|v| v.sun_color))
            .unwrap_or([0.30, 0.30, 0.30]);
        env.set_ambient(Vec3::from_array(ambient));

        let direction = lighting
            .and_then(|v| v.sun_direction)
            .unwrap_or([-0.5, -1.0, -0.5]);
        let color = lighting
            .and_then(|v| v.sun_color)
            .or_else(|| lighting.and_then(|v| v.fog_color))
            .or_else(|| lighting.and_then(|v| v.ambient_color))
            .unwrap_or([1.0, 0.9, 0.8]);

        let direction = Vec3::from_array(direction).normalize_or_zero();
        let mut light = LightClass::directional(direction, Vec3::from_array(color), 1.0);
        light.enabled = true;
        env.add_light(Arc::new(Mutex::new(light)));

        #[cfg(feature = "game_client")]
        {
            for pulse in game_client::fx_list::scene_dynamic_lights() {
                if !pulse.enabled {
                    continue;
                }
                // C++ map XYZ (z-up) → wgpu Y-up.
                let position = Vec3::new(pulse.pos[0], pulse.pos[2], pulse.pos[1]);
                let pulse_color = Vec3::from_array(pulse.color);
                let range = pulse.far_atten_end.max(pulse.far_atten_start).max(0.1);
                let intensity = pulse_color.max_element().max(0.01);
                let mut point = LightClass::point(position, pulse_color, intensity, range);
                point.enabled = true;
                env.add_light(Arc::new(Mutex::new(point)));
            }
        }

        static LOGGED_FALLBACK_LIGHTING: AtomicBool = AtomicBool::new(false);
        if !have_metadata && !LOGGED_FALLBACK_LIGHTING.swap(true, Ordering::Relaxed) {
            warn!(
                "ForwardPass lighting metadata unavailable/incomplete; using fallback ambient+sun lighting"
            );
        }
        Some(env)
    }

    pub(super) fn log_visibility_probe(
        render_items: &[RenderItem],
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
    ) {
        static PROBE_FRAME_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let frame = PROBE_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        if !frame.is_multiple_of(120) {
            return;
        }

        let sample_limit = render_items.len().min(512);
        let view_proj = *projection_matrix * *view_matrix;
        let mut finite = 0usize;
        let mut in_front = 0usize;
        let mut in_ndc = 0usize;
        let mut ndc_samples: Vec<String> = Vec::new();

        for item in render_items.iter().take(sample_limit) {
            let world = item.world_position.extend(1.0);
            let clip = view_proj * world;
            if !clip.x.is_finite()
                || !clip.y.is_finite()
                || !clip.z.is_finite()
                || !clip.w.is_finite()
            {
                continue;
            }
            finite += 1;
            if clip.w <= 0.0 {
                continue;
            }
            in_front += 1;

            let inv_w = 1.0 / clip.w;
            let ndc = clip * inv_w;
            if ndc.x >= -1.2
                && ndc.x <= 1.2
                && ndc.y >= -1.2
                && ndc.y <= 1.2
                && ndc.z >= -0.2
                && ndc.z <= 1.2
            {
                in_ndc += 1;
                if ndc_samples.len() < 3 {
                    ndc_samples.push(format!(
                        "{} ndc=({:.2},{:.2},{:.2}) world=({:.1},{:.1},{:.1})",
                        item.model_name,
                        ndc.x,
                        ndc.y,
                        ndc.z,
                        item.world_position.x,
                        item.world_position.y,
                        item.world_position.z
                    ));
                }
            }
        }

        debug!(
            "VisibilityProbe frame={} items={} sampled={} finite={} in_front={} in_ndc={} cam=({:.1},{:.1},{:.1}) sample={:?}",
            frame,
            render_items.len(),
            sample_limit,
            finite,
            in_front,
            in_ndc,
            camera_position.x,
            camera_position.y,
            camera_position.z,
            ndc_samples
        );

        if sample_limit > 0 && in_front > 0 && in_ndc == 0 {
            warn!(
                "VisibilityProbe anomaly: no items in NDC despite in_front={} sampled={} (cam=({:.1},{:.1},{:.1})) sample={:?}",
                in_front,
                sample_limit,
                camera_position.x,
                camera_position.y,
                camera_position.z,
                ndc_samples
            );
        }
    }

    pub(super) fn log_material_probe(render_items: &[RenderItem]) {
        static MATERIAL_PROBE_FRAME_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let frame = MATERIAL_PROBE_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        if !frame.is_multiple_of(120) {
            return;
        }

        let sample_limit = render_items.len().min(512);
        let mut textured = 0usize;
        let mut near_black_diffuse = 0usize;
        let mut near_zero_opacity = 0usize;
        let mut emissive = 0usize;
        let mut samples: Vec<String> = Vec::new();

        for item in render_items.iter().take(sample_limit) {
            let mat = &item.material;
            if mat.texture_name.is_some() {
                textured += 1;
            }
            let max_diffuse = mat
                .diffuse_color
                .x
                .max(mat.diffuse_color.y)
                .max(mat.diffuse_color.z);
            if max_diffuse <= 0.02 {
                near_black_diffuse += 1;
            }
            if mat.opacity <= 0.02 {
                near_zero_opacity += 1;
            }
            if mat.emissive_color.length_squared() > 0.0001 {
                emissive += 1;
            }

            if samples.len() < 3 {
                samples.push(format!(
                    "{} tex={:?} diffuse=({:.2},{:.2},{:.2}) opacity={:.2} blend={:?}",
                    mat.name,
                    mat.texture_name,
                    mat.diffuse_color.x,
                    mat.diffuse_color.y,
                    mat.diffuse_color.z,
                    mat.opacity,
                    mat.blend_mode
                ));
            }
        }

        debug!(
            "MaterialProbe frame={} items={} sampled={} textured={} black_diffuse={} zero_opacity={} emissive={} sample={:?}",
            frame,
            render_items.len(),
            sample_limit,
            textured,
            near_black_diffuse,
            near_zero_opacity,
            emissive,
            samples
        );

        if sample_limit > 0
            && (near_zero_opacity * 100 / sample_limit >= 90
                || near_black_diffuse * 100 / sample_limit >= 90)
        {
            warn!(
                "MaterialProbe anomaly: mostly non-visible materials (sampled={} textured={} black_diffuse={} zero_opacity={} emissive={}) sample={:?}",
                sample_limit,
                textured,
                near_black_diffuse,
                near_zero_opacity,
                emissive,
                samples
            );
        }
    }

    pub(super) fn prepare_mesh_instance(
        &mut self,
        graphics_system: &GraphicsSystem,
        item: &RenderItem,
    ) -> Result<Option<Arc<MeshClass>>> {
        let mesh_model = match self.ensure_mesh_model(graphics_system, item)? {
            Some(model) => model,
            None => return Ok(None),
        };

        let mut mesh = MeshClass::new();
        mesh.set_transform(item.world_matrix * item.mesh_local_transform);
        mesh.model = Some(mesh_model);
        mesh.alpha_override = item.fow_visibility.visibility_alpha;
        mesh.is_hidden = item.fow_visibility.visibility_alpha <= 0.01;
        mesh.set_uv_offset_override(item.uv_offset_override.map(|offset| [offset.x, offset.y]));
        if std::env::var_os("GENERALS_FORCE_TWO_SIDED").is_some() {
            static LOGGED_FORCE_TWO_SIDED: AtomicBool = AtomicBool::new(false);
            if !LOGGED_FORCE_TWO_SIDED.swap(true, Ordering::Relaxed) {
                warn!("GENERALS_FORCE_TWO_SIDED enabled: forcing two-sided pipelines for mesh diagnostics");
            }
            mesh.is_decal_instance = true;
        }

        if let Some(w3d_model) = graphics_system.get_model(&item.model_name) {
            if !w3d_model.animations.is_empty() && w3d_model.hierarchy.is_some() {
                if let Some(bone_transforms) = w3d_model.sample_animation(0, item.animation_frame) {
                    let matrices: Vec<Mat4> =
                        bone_transforms.iter().map(Mat4::from_cols_array).collect();
                    mesh.set_bone_palette_slice(&matrices);
                }
            }
        }

        Ok(Some(Arc::new(mesh)))
    }

    pub(super) fn enqueue_post_frame_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.renderer.enqueue_post_frame_callback(callback);
    }

    pub(super) fn enqueue_pre_scene_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.renderer.enqueue_pre_scene_callback(callback);
    }

    pub(super) fn ensure_mesh_model(
        &mut self,
        graphics_system: &GraphicsSystem,
        item: &RenderItem,
    ) -> Result<Option<Arc<MeshModelClass>>> {
        let cache_key = format!(
            "{}::{}::{}",
            item.model_name, item.mesh_index, item.material_key
        );

        if let Some(model) = self.mesh_cache.get(&cache_key) {
            return Ok(Some(model.clone()));
        }

        let w3d_model = match graphics_system.get_model(&item.model_name) {
            Some(model) => Arc::clone(model),
            None => {
                warn!("No cached W3D model for '{}'", item.model_name);
                return Ok(None);
            }
        };

        let mesh = match w3d_model.meshes.get(item.mesh_index) {
            Some(mesh) => mesh,
            None => {
                warn!(
                    "Model '{}' missing mesh index {}",
                    item.model_name, item.mesh_index
                );
                return Ok(None);
            }
        };

        if let Some(mesh_model) = w3d_model.ww3d_mesh_models.get(&mesh.name) {
            let mesh_model = Arc::clone(mesh_model);
            self.mesh_cache.insert(cache_key, mesh_model.clone());
            return Ok(Some(mesh_model));
        }

        let mesh_model = Arc::new(self.build_mesh_model(&cache_key, mesh, &item.material)?);
        self.mesh_cache.insert(cache_key, mesh_model.clone());
        Ok(Some(mesh_model))
    }
}
