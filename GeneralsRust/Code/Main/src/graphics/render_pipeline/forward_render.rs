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
use crate::graphics::fow_uniform_integration::frozen_fow_model_fields;
use crate::graphics::render_item::RenderItemBonePaletteSource;
use ww3d_renderer_3d::rendering::mesh_system::FrozenFowVisibility;

#[cfg(feature = "game_client")]
use game_client::effects::particle_renderer::{ParticleUniforms, register_particle_renderer};
#[cfg(feature = "game_client")]
use game_client::effects::weather_complete::get_weather_system;
#[cfg(feature = "game_client")]
use game_client::effects::{ParticleRenderer, get_particle_system_manager};
#[cfg(feature = "game_client")]
use game_client::fx_list::get_decal_manager;
#[cfg(feature = "game_client")]
use game_client::radius_decal::get_projected_shadow_manager;
#[cfg(feature = "game_client")]
use game_client::system::smudge::get_smudge_manager;

impl ForwardPass {
    /// Produce the GPU palette from the exact binding frozen on the render
    /// item. This compatibility helper can only resolve the item's own model;
    /// an HMODEL source key that differs from `item.model_name` fails closed.
    /// The live forward path uses
    /// [`Self::sample_bone_palette_for_item_with_model_resolver`] so a strict
    /// child mesh can retain a separately owned HMODEL palette source.
    pub(super) fn sample_bone_palette_for_item(
        w3d_model: &crate::assets::W3DModel,
        item: &RenderItem,
    ) -> Option<Vec<Mat4>> {
        match &item.bone_palette_source {
            RenderItemBonePaletteSource::FrozenDrawState => w3d_model
                .animation_palette_for_binding_and_capture_controls(
                    item.animation_binding.as_ref(),
                    item.animation_frame,
                    &item.capture_bone_controls,
                ),
            RenderItemBonePaletteSource::HierarchyBindPose => w3d_model
                .animation_palette_for_binding_and_capture_controls(
                    item.animation_binding.as_ref(),
                    item.animation_frame,
                    &item.capture_bone_controls,
                )
                .or_else(|| sample_resolved_hierarchy_bind_pose_palette(w3d_model)),
            RenderItemBonePaletteSource::HmodelBindPose {
                source_model_cache_key,
                hmodel_index,
            } => (source_model_cache_key == &item.model_name)
                .then(|| w3d_model.hmodel_bind_pose_palette(*hmodel_index))
                .flatten(),
        }
    }

    /// Resolve the exact model whose source owns this item's palette.
    ///
    /// This is deliberately keyed by the explicit `RenderItem` palette
    /// source, not by the draw mesh's model. An HMODEL SKIN_NODE can resolve a
    /// strict mesh token from another source file while its C++ `MeshClass`
    /// container remains the HMODEL's own named/default HTree.
    pub(super) fn sample_bone_palette_for_item_with_model_resolver<'model, F>(
        item: &RenderItem,
        mut resolve_model: F,
    ) -> Option<Vec<Mat4>>
    where
        F: FnMut(&str) -> Option<&'model crate::assets::W3DModel>,
    {
        match &item.bone_palette_source {
            RenderItemBonePaletteSource::FrozenDrawState => {
                let w3d_model = resolve_model(&item.model_name)?;
                w3d_model.animation_palette_for_binding_and_capture_controls(
                    item.animation_binding.as_ref(),
                    item.animation_frame,
                    &item.capture_bone_controls,
                )
            }
            RenderItemBonePaletteSource::HierarchyBindPose => {
                let w3d_model = resolve_model(&item.model_name)?;
                w3d_model
                    .animation_palette_for_binding_and_capture_controls(
                        item.animation_binding.as_ref(),
                        item.animation_frame,
                        &item.capture_bone_controls,
                    )
                    .or_else(|| sample_resolved_hierarchy_bind_pose_palette(w3d_model))
            }
            RenderItemBonePaletteSource::HmodelBindPose {
                source_model_cache_key,
                hmodel_index,
            } => resolve_model(source_model_cache_key)?.hmodel_bind_pose_palette(*hmodel_index),
        }
    }

    /// Validate the final renderer mesh against an HMODEL-owned palette.
    /// Collection already validates the immutable W3D source mesh; this last
    /// check also rejects a stale or incompatible prebuilt `MeshModelClass`
    /// instead of letting its fallback bone data render with a wrong palette.
    fn hmodel_skin_mesh_matches_palette(mesh_model: &MeshModelClass, palette: &[Mat4]) -> bool {
        !palette.is_empty()
            && mesh_model.is_skinned()
            && mesh_model.vertex_influences().is_some_and(|influences| {
                !influences.is_empty()
                    && influences
                        .iter()
                        .all(|influence| usize::from(influence.bone_idx) < palette.len())
            })
    }

    pub(super) fn initialize(graphics_system: &GraphicsSystem) -> Result<Self> {
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

        #[cfg(feature = "game_client")]
        let particle_renderer = match graphics_system.depth_format() {
            Some(depth_format) => match ParticleRenderer::new_with_depth_format(
                Arc::clone(&device),
                Arc::clone(&queue),
                graphics_system.color_format(),
                depth_format,
            ) {
                Ok(renderer) => {
                    let renderer = Arc::new(Mutex::new(renderer));
                    // Asset loading happens after RenderPipeline initialization, so
                    // registering Main's sole renderer lets normal GameClient asset
                    // uploads populate this exact WGPU device instead of a second
                    // Display-owned surface.
                    register_particle_renderer(Arc::clone(&renderer));
                    Some(renderer)
                }
                Err(error) => {
                    warn!("Main particle renderer initialization failed: {error}");
                    None
                }
            },
            None => {
                warn!("Main frame has no depth target; particle draw is unavailable");
                None
            }
        };

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
            projected_shroud_uploader: crate::graphics::ProjectedShroudGpuUploader::default(),
            ghost_lighting_environment: None,
            tactical_view_height_frac: 1.0,
            laser_vertex_buffer: None,
            laser_vertex_capacity: 0,
            laser_vertices_uploaded: 0,
            laser_draw_gpu: None,
            #[cfg(feature = "game_client")]
            particle_renderer,
        })
    }

    /// Upload packed laser SegLine vertices with `Queue::write_buffer`.
    ///
    /// Creates/resizes the vertex buffer when needed. Empty packs return false
    /// and do not claim a live write.
    pub(super) fn upload_laser_segments(
        &mut self,
        upload: &mut crate::graphics::laser_segment_upload::LaserSegmentUpload,
    ) -> bool {
        if !upload.vertex_bytes.is_empty() && upload.honesty.cpu_pack_ok {
            let needed = upload.vertex_bytes.len().max(16);
            let need_new = self
                .laser_vertex_buffer
                .as_ref()
                .map(|_| needed > self.laser_vertex_capacity)
                .unwrap_or(true);
            if need_new {
                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("laser_segliner_vertex"),
                    size: needed as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.laser_vertex_buffer = Some(Arc::new(buffer));
                self.laser_vertex_capacity = needed;
            }
        }
        // Sole writer of gpu_write_buffer_submitted is write_to_queue.
        if let Some((bytes, buffer)) =
            upload.write_to_queue(self.laser_vertex_buffer.as_ref().map(|b| b.as_ref()))
        {
            self.queue.write_buffer(buffer, 0, &bytes);
            self.laser_vertices_uploaded = upload.vertex_count();
            true
        } else {
            self.laser_vertices_uploaded = 0;
            false
        }
    }

    /// Enqueue the live additive SegLine draw for vertices just uploaded.
    ///
    /// The draw call lives in [`crate::graphics::laser_draw::LaserDrawGpu::draw`]
    /// and is issued from a post-frame callback so lasers are visible.
    pub(super) fn enqueue_laser_additive_draw(
        &mut self,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
    ) -> bool {
        let vertex_count = self.laser_vertices_uploaded;
        let Some(buffer) = self.laser_vertex_buffer.clone() else {
            return false;
        };
        if vertex_count < 2 {
            return false;
        }
        if self.laser_draw_gpu.is_none() {
            self.laser_draw_gpu = Some(crate::graphics::laser_draw::LaserDrawGpu::new(
                self.device.as_ref(),
                wgpu::TextureFormat::Bgra8UnormSrgb,
            ));
        }
        let view_proj = *projection_matrix * *view_matrix;
        let Some(gpu) = self.laser_draw_gpu.as_ref() else {
            return false;
        };
        self.queue.write_buffer(
            gpu.camera_buffer.as_ref(),
            0,
            bytemuck::bytes_of(&view_proj),
        );
        let pipeline = gpu.pipeline.clone();
        let camera_bg = gpu.camera_bind_group.clone();
        // Capture enough to draw after the 3D scene.
        self.renderer.enqueue_post_frame_callback(move |frame| {
            let color_view = frame.color_view_arc();
            let depth_view = frame.depth_view_arc();
            let encoder = frame.encoder();
            let depth_stencil =
                depth_view
                    .as_ref()
                    .map(|dv| wgpu::RenderPassDepthStencilAttachment {
                        view: dv.as_ref(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    });
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("laser_additive_segliner"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_stencil,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(pipeline.as_ref());
            render_pass.set_bind_group(0, Some(camera_bg.as_ref()), &[]);
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.draw(0..vertex_count, 0..1);
            drop(render_pass);
            Ok(())
        });
        true
    }

    /// Queue GameClient particle, weather, and decal draws on Main's sole WGPU
    /// frame.  `GameClient::Display::draw` stays disconnected: it would create
    /// and present a second surface instead of sharing this post-scene target.
    #[cfg(feature = "game_client")]
    pub(super) fn enqueue_client_effect_draw(
        &mut self,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        time: f32,
    ) -> bool {
        let Some(renderer) = self.particle_renderer.as_ref().map(Arc::clone) else {
            return false;
        };

        // Leftover ParticleSystemManager / tracers / weather / decals are C++
        // Z-up. C++ W3DDisplay::DoParticles uses TheTacticalView (same Z-up).
        // Do not submit leftover world pos through the live Y-up view_matrix.
        let uniforms = leftover_z_up_particle_uniforms(time);
        let _ = (view_matrix, projection_matrix, camera_position);

        self.renderer.enqueue_post_frame_callback(move |frame| {
            let Some(depth_view) = frame.depth_view_arc() else {
                // The renderer was constructed for a depth-backed Main frame;
                // do not issue a depth-test pipeline against an absent target.
                return Ok(());
            };
            let color_view = frame.color_view_arc();
            let color_texture = frame.color_texture().clone();
            let encoder = frame.encoder();

            let Ok(mut renderer_guard) = renderer.lock() else {
                warn!("Main particle renderer lock poisoned; skipping effect draw");
                return Ok(());
            };

            // Systems are owned and advanced by the GameClient presentation
            // shell. Borrow the real manager only while its live system refs are
            // submitted; no Main GameLogic state is read in this render callback.
            if let Ok(mut manager_guard) = game_client::effects::get_particle_system_manager_mut() {
                if let Some(manager) = manager_guard.as_mut() {
                    game_client::display::view::with_tactical_view_ref(|view| {
                        let cam = view.get_3d_camera_position();
                        let target = view.position();
                        let aspect = (view.width() as f32 / view.height().max(1) as f32).max(0.01);
                        let visible = game_client::display::shadow_pass::maximum_visible_box(
                            [cam.x, cam.y, cam.z],
                            [target.x, target.y, target.z],
                            1.0,
                            20000.0,
                            game_client::display::view::vertical_fov_from_horizontal(
                                view.field_of_view(),
                                aspect,
                            ),
                            aspect,
                            game_client::display::shadow_pass::terrain_min_height(),
                        );
                        manager.cull_particles_to_visible_box(visible.center, visible.extent, 512);
                    });
                }
            }
            if let Ok(manager_guard) = get_particle_system_manager() {
                if let Some(manager) = manager_guard.as_ref() {
                    let systems = manager.draw_particle_systems();
                    if !systems.is_empty() {
                        let mut particle_uniforms = uniforms;
                        particle_uniforms.particle_count = systems
                            .iter()
                            .map(|system| system.particle_count())
                            .sum::<usize>()
                            .min(u32::MAX as usize)
                            as u32;
                        renderer_guard.render_particles(
                            encoder,
                            color_view.as_ref(),
                            depth_view.as_ref(),
                            &systems,
                            &particle_uniforms,
                        );
                    }
                }
            }

            // C++ W3DDisplay draws FXList Tracer/RayEffect after particles
            // (Display::draw). Host presentation skips Display::draw, so the
            // same submit must happen on Main's sole present path.
            renderer_guard.render_tracer_and_ray_fx(
                encoder,
                color_view.as_ref(),
                depth_view.as_ref(),
                &uniforms,
            );

            // Weather and decals are updated by `update_presentation_shell` too.
            // Draw their real client-managed data in the same post-scene order as
            // GameClient::Display rather than synthesizing presentation geometry.
            if let Ok(weather_guard) = get_weather_system() {
                if let Some(weather) = weather_guard.as_ref() {
                    let particles = weather.get_all_particles();
                    if !particles.is_empty() {
                        let mut weather_uniforms = uniforms;
                        weather_uniforms.particle_count =
                            particles.len().min(u32::MAX as usize) as u32;
                        renderer_guard.render_weather_particles(
                            encoder,
                            color_view.as_ref(),
                            depth_view.as_ref(),
                            &particles,
                            &weather_uniforms,
                        );
                    }
                }
            }

            let mut decals = if let Some(manager) = get_decal_manager() {
                manager
                    .lock()
                    .map(|guard| guard.collect_render_items())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            // C++ W3DProjectedShadowManager::flushDecals — radius addDecal rings
            // stay ungated. Unit addShadow blobs are omitted when 2D Shadows off
            // (`collect_render_items` / W3DProjectedShadow.cpp:1303).
            decals.extend(get_projected_shadow_manager().read().collect_render_items());

            if !decals.is_empty() {
                let mut decal_uniforms = uniforms;
                decal_uniforms.particle_count = decals.len().min(u32::MAX as usize) as u32;
                renderer_guard.render_decals(
                    encoder,
                    color_view.as_ref(),
                    depth_view.as_ref(),
                    &decals,
                    &decal_uniforms,
                );
            }

            if let Ok(manager) = get_smudge_manager().lock() {
                let smudges = game_client::effects::heat_haze::collect_heat_haze_smudges(
                    &manager.collect_used_smudges(),
                );
                if !smudges.is_empty() {
                    renderer_guard.render_heat_haze(
                        encoder,
                        color_view.as_ref(),
                        depth_view.as_ref(),
                        Some(&color_texture),
                        &smudges,
                        &uniforms,
                    );
                }
            }

            Ok(())
        });
        true
    }

    /// Check if the forward pass is ready to render
    /// Returns true if all required resources are available
    pub(super) fn is_ready(&self) -> bool {
        // Verify engine is still initialized by checking if we can get device/queue
        // The Arc references we hold should still be valid, but engine might have shut down
        ww3d_engine::device().is_ok() && ww3d_engine::queue().is_ok()
    }

    #[allow(unused_assignments)]
    pub(super) fn prewarm_textures_blocking<I, S>(
        &mut self,
        texture_names: I,
    ) -> Result<TexturePrewarmStats>
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
        projected_shroud: Option<&crate::fow_rendering::ProjectedShroudSnapshot>,
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

        // Freeze GPU resource state before the renderer frame. An absent or
        // inactive presentation snapshot explicitly releases stale map data.
        let inactive_projected_shroud;
        let projected_shroud = match projected_shroud {
            Some(snapshot) => snapshot,
            None => {
                inactive_projected_shroud =
                    crate::fow_rendering::ProjectedShroudSnapshot::inactive();
                &inactive_projected_shroud
            }
        };
        self.projected_shroud_uploader.sync(
            self.device.as_ref(),
            self.queue.as_ref(),
            projected_shroud,
        );
        let projected_shroud_binding = self
            .projected_shroud_uploader
            .renderer_binding(projected_shroud);
        self.ghost_lighting_environment =
            Self::build_always_fogged_light_environment(lighting).map(Arc::new);

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
            let mut renderer = match renderer_handle.try_lock() {
                Ok(guard) => guard,
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    // C++ winRepaint still runs after a 3D fault. End the
                    // WW3D frame so pending_frame is not leaked and the UI
                    // overlay still flushes.
                    self.renderer.enqueue_post_frame_callback(|frame| {
                        crate::graphics::ui_render_pass::flush_ui_to_frame(frame)
                    });
                    let _ = self.renderer.end_frame();
                    return Err(anyhow::anyhow!(
                        "WW3D renderer handle poisoned - another thread panicked while holding the lock"
                    ));
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    warn!("WW3D renderer handle already locked - skipping mesh queue");
                    self.renderer.enqueue_post_frame_callback(|frame| {
                        crate::graphics::ui_render_pass::flush_ui_to_frame(frame)
                    });
                    self.renderer.end_frame().map_err(|e| {
                        anyhow::anyhow!(
                            "WW3D renderer end_frame failed after lock contention: {e:?}"
                        )
                    })?;
                    return Ok(());
                }
            };

            // Update camera state - must happen before queueing meshes.
            // Order matters: set_position marks the camera dirty and would
            // rebuild the just-installed matrices from view-plane/clip
            // defaults (90° FOV, near 1, far 1000), so it must run FIRST.
            // The explicit view/projection are installed last and stay
            // authoritative (C++ CameraClass::Apply parity: the D3D
            // projection built from ViewPlane/clip planes is what renders).
            self.camera.set_position(camera_position);
            self.camera.set_view_matrix(*view_matrix);
            self.camera.set_projection_matrix(*projection_matrix);
            let frac = self.tactical_view_height_frac.clamp(0.05, 1.0);
            self.camera
                .set_viewport(glam::Vec2::new(0.0, 0.0), glam::Vec2::new(1.0, frac));
            renderer.set_camera(self.camera.clone());
            // UTBTERRVP (GENERALS_UTBVP=1, once): dump the terrain-lane
            // view/projection next to the just-installed ww3d camera's cached
            // view-projection; the two must agree numerically or the mesh lane
            // is drawing through a different transform than the terrain.
            static UTBTERRVP_DONE: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if std::env::var("GENERALS_UTBVP").as_deref() == Ok("1")
                && !UTBTERRVP_DONE.swap(true, std::sync::atomic::Ordering::Relaxed)
            {
                let tv = view_matrix.to_cols_array();
                let tp = projection_matrix.to_cols_array();
                let cvp = self.camera.get_cached_view_projection_matrix().to_cols_array();
                let terr_vp = (*projection_matrix * *view_matrix).to_cols_array();
                log::warn!(
                    "UTBTERRVP view=[{:.4},{:.4},{:.4},{:.4} / {:.4},{:.4},{:.4},{:.4} / {:.4},{:.4},{:.4},{:.4} / {:.4},{:.4},{:.4},{:.4}]",
                    tv[0], tv[1], tv[2], tv[3], tv[4], tv[5], tv[6], tv[7],
                    tv[8], tv[9], tv[10], tv[11], tv[12], tv[13], tv[14], tv[15],
                );
                log::warn!(
                    "UTBTERRVP proj=[{:.4},{:.4},{:.4},{:.4} / {:.4},{:.4},{:.4},{:.4} / {:.4},{:.4},{:.4},{:.4} / {:.4},{:.4},{:.4},{:.4}]",
                    tp[0], tp[1], tp[2], tp[3], tp[4], tp[5], tp[6], tp[7],
                    tp[8], tp[9], tp[10], tp[11], tp[12], tp[13], tp[14], tp[15],
                );
                log::warn!(
                    "UTBTERRVP terr_vp0=({:.4},{:.4},{:.4},{:.4}) cam_vp0=({:.4},{:.4},{:.4},{:.4}) vp_row3=({:.4},{:.4},{:.4},{:.4})",
                    terr_vp[0], terr_vp[1], terr_vp[2], terr_vp[3],
                    cvp[0], cvp[1], cvp[2], cvp[3],
                    cvp[12], cvp[13], cvp[14], cvp[15],
                );
            }
            renderer.set_light_environment(Self::build_light_environment(lighting));
            renderer.set_projected_shroud(projected_shroud_binding);
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
            let renderable_passes = [
                RenderPass::ForwardOpaque,
                RenderPass::ForwardTransparent,
                RenderPass::Ghost,
            ];

            for item in render_items {
                if !renderable_passes.contains(&item.render_pass) {
                    continue;
                }
                let is_ghost =
                    item.render_pass == RenderPass::Ghost || item.ghost_render_state.is_some();
                if !is_ghost && item.fow_visibility.visibility_alpha <= 0.01 {
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

        // C++ W3DInGameUI::draw always winRepaints. If render_frame fails,
        // post-frame callbacks are dropped. Enqueue UI as last pre-scene so
        // the overlay still presents over terrain, and again as post-frame
        self.renderer.enqueue_pre_scene_callback(|frame| {
            if let Err(err) = crate::graphics::ui_render_pass::flush_ui_to_frame(frame) {
                log::warn!("pre-scene flush_ui_to_frame failed: {err}");
            }
            Ok(())
        });
        self.renderer.enqueue_post_frame_callback(|frame| {
            if let Err(err) = crate::graphics::ui_render_pass::flush_ui_to_frame(frame) {
                log::warn!("post-frame flush_ui_to_frame failed: {err}");
            }
            Ok(())
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

    pub(super) fn build_light_environment(
        lighting: Option<&CachedLighting>,
    ) -> Option<LightEnvironmentClass> {
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

    /// Build the C++ `m_foggedLightEnv` equivalent for W3D ghosts. The ratio
    /// is frozen from GlobalData at the presentation boundary; no live FOW or
    /// GameLogic query is permitted here.
    pub(super) fn build_always_fogged_light_environment(
        lighting: Option<&CachedLighting>,
    ) -> Option<LightEnvironmentClass> {
        let mut env = Self::build_light_environment(lighting)?;
        let fraction = lighting
            .and_then(|value| value.fogged_light_fraction)
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        env.ambient *= fraction;
        for light in &env.lights {
            if let Ok(mut light) = light.lock() {
                light.color *= fraction;
            }
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
                sample_limit, textured, near_black_diffuse, near_zero_opacity, emissive, samples
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

        let palette = Self::sample_bone_palette_for_item_with_model_resolver(item, |model_key| {
            graphics_system
                .get_model(model_key)
                .map(|model| model.as_ref())
        });
        if matches!(
            &item.bone_palette_source,
            RenderItemBonePaletteSource::HmodelBindPose { .. }
                | RenderItemBonePaletteSource::HierarchyBindPose
        ) {
            let Some(palette) = palette.as_deref() else {
                // An explicit skin owner is not optional. Rendering it
                // without the exact source HTree would silently substitute a
                // parent, whole-file, or renderer identity-pad palette.
                return Ok(None);
            };
            if matches!(
                &item.bone_palette_source,
                RenderItemBonePaletteSource::HmodelBindPose { .. }
            ) && !Self::hmodel_skin_mesh_matches_palette(mesh_model.as_ref(), palette)
            {
                return Ok(None);
            }
        }

        let mut mesh = MeshClass::new();
        mesh.set_transform(item.world_matrix * item.mesh_local_transform);
        mesh.model = Some(mesh_model);
        // FOW was captured on the RenderItem during collection. The draw path
        // must carry that snapshot, not consult GameLogic/FOW again.
        let is_ghost = item.render_pass == RenderPass::Ghost
            || item
                .ghost_render_state
                .as_ref()
                .is_some_and(|state| state.lighting_route == GhostLightingRoute::AlwaysFogged);
        if is_ghost {
            mesh.set_lighting_environment(self.ghost_lighting_environment.clone());
            mesh.set_projected_shroud_eligible(false);
        } else {
            let frozen_fow = frozen_fow_model_fields(item.fow_visibility);
            mesh.set_frozen_fow_visibility(FrozenFowVisibility::new(
                frozen_fow.visibility_alpha,
                frozen_fow.visibility_falloff,
                frozen_fow.is_explored,
            ));
            mesh.set_projected_shroud_eligible(item.pushes_projected_shroud_pass());
            mesh.alpha_override = item.fow_visibility.visibility_alpha;
            mesh.is_hidden = item.fow_visibility.visibility_alpha <= 0.01;
        }
        mesh.set_presentation_opacity(item.presentation_opacity);
        // C++ `dx8renderer.cpp:1854-1886` Material_Override: HLOD ghosts force
        // LinearOffset to customUVOffset(0,0) for this draw only. The shared
        // Arc<W3DModel> mapper config is left untouched for live items.
        let uv_override = if is_ghost
            && item
                .ghost_render_state
                .as_ref()
                .is_some_and(|state| state.uv_animations_disabled)
        {
            Some([0.0, 0.0])
        } else {
            item.uv_offset_override.map(|offset| [offset.x, offset.y])
        };
        mesh.set_uv_offset_override(uv_override);
        if std::env::var_os("GENERALS_FORCE_TWO_SIDED").is_some() {
            static LOGGED_FORCE_TWO_SIDED: AtomicBool = AtomicBool::new(false);
            if !LOGGED_FORCE_TWO_SIDED.swap(true, Ordering::Relaxed) {
                warn!(
                    "GENERALS_FORCE_TWO_SIDED enabled: forcing two-sided pipelines for mesh diagnostics"
                );
            }
            mesh.is_decal_instance = true;
        }

        if let Some(matrices) = palette {
            mesh.set_bone_palette_slice(&matrices);
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

/// C++ `HTreeClass::Base_Update` for the HLOD-named HTree, converted to the
/// same render basis `W3DModel` uses for rigid children and HAnim palettes.
///
/// `animation_palette_for_binding_and_capture_controls` deliberately returns
/// `None` for a bind-pose Draw state so rigid items do not upload clip zero.
/// SKIN items stamp [`RenderItemBonePaletteSource::HierarchyBindPose`] and
/// fall through here instead of the renderer's identity 64-mat pad.
fn sample_resolved_hierarchy_bind_pose_palette(
    w3d_model: &crate::assets::W3DModel,
) -> Option<Vec<Mat4>> {
    if w3d_model.hlod_parse_failed {
        return None;
    }
    let hierarchy = match w3d_model.hlods.len() {
        0 => w3d_model.hierarchy.as_ref()?,
        1 => {
            let hlod = w3d_model.hlods.first()?;
            if hlod.has_invalid_trailing_records || hlod.hierarchy_name.is_empty() {
                return None;
            }
            w3d_model
                .hierarchies
                .iter()
                .find(|hierarchy| {
                    hierarchy
                        .name
                        .eq_ignore_ascii_case(hlod.hierarchy_name.as_str())
                })
                .or_else(|| {
                    w3d_model.hierarchy.as_ref().filter(|hierarchy| {
                        hierarchy
                            .name
                            .eq_ignore_ascii_case(hlod.hierarchy_name.as_str())
                    })
                })?
        }
        _ => return None,
    };
    if hierarchy.pivots.is_empty() || hierarchy.pivots[0].parent_idx != u32::MAX {
        return None;
    }

    let locals: Vec<Mat4> = hierarchy.pivots.iter().map(pivot_local_transform).collect();
    let mut globals = vec![Mat4::IDENTITY; hierarchy.pivots.len()];
    for (pivot_index, pivot) in hierarchy.pivots.iter().enumerate().skip(1) {
        let parent_index = usize::try_from(pivot.parent_idx).ok()?;
        if parent_index >= pivot_index {
            return None;
        }
        globals[pivot_index] = globals[parent_index] * locals[pivot_index];
    }
    Some(
        globals
            .into_iter()
            .map(w3d_source_transform_to_render_basis)
            .collect(),
    )
}

fn pivot_local_transform(pivot: &crate::assets::W3dPivot) -> Mat4 {
    let x = pivot.rotation[0];
    let y = pivot.rotation[1];
    let z = pivot.rotation[2];
    let w = pivot.rotation[3];
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    Mat4::from_cols_array(&[
        1.0 - 2.0 * (yy + zz),
        2.0 * (xy + wz),
        2.0 * (xz - wy),
        0.0,
        2.0 * (xy - wz),
        1.0 - 2.0 * (xx + zz),
        2.0 * (yz + wx),
        0.0,
        2.0 * (xz + wy),
        2.0 * (yz - wx),
        1.0 - 2.0 * (xx + yy),
        0.0,
        pivot.translation[0],
        pivot.translation[1],
        pivot.translation[2],
        1.0,
    ])
}

#[cfg(feature = "game_client")]
fn leftover_z_up_particle_uniforms(time: f32) -> ParticleUniforms {
    game_client::display::view::with_tactical_view_ref(|view| {
        let cam = view.get_3d_camera_position();
        let target = view.position();
        let camera = Vec3::new(cam.x, cam.y, cam.z);
        let look = Vec3::new(target.x, target.y, target.z);
        let view_matrix = Mat4::look_at_rh(camera, look, Vec3::Z);
        let aspect = (view.width() as f32 / view.height().max(1) as f32).max(0.01);
        let fov =
            game_client::display::view::vertical_fov_from_horizontal(view.field_of_view(), aspect);
        let projection_matrix = Mat4::perspective_rh(fov, aspect, 1.0, 20000.0);
        ParticleUniforms {
            view_matrix: view_matrix.to_cols_array_2d(),
            projection_matrix: projection_matrix.to_cols_array_2d(),
            camera_position: [camera.x, camera.y, camera.z],
            time,
            ..ParticleUniforms::default()
        }
    })
}

fn w3d_source_transform_to_render_basis(transform: Mat4) -> Mat4 {
    let axis = Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    axis * transform * axis
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{W3DMaterial, W3DModel, W3dHierarchy, W3dHmodel, W3dPivot};
    use crate::game_logic::ObjectId;
    use crate::graphics::render_item::{RenderItem, RenderItemBonePaletteSource};
    use std::collections::HashMap;

    #[test]
    fn leftover_particles_present_with_z_up_view() {
        let src = include_str!("forward_render.rs");
        assert!(
            src.contains("fn leftover_z_up_particle_uniforms")
                && src.contains("Mat4::look_at_rh(camera, look, Vec3::Z)")
                && src.contains("leftover_z_up_particle_uniforms(time)"),
            "Main present must submit leftover Z-up particles with leftover Z-up view"
        );
        assert!(
            !src.contains("camera_position: camera_position.to_array()"),
            "leftover ParticleSystemManager must not use the live Y-up camera_position"
        );
    }

    fn pivot(name: &str, parent_idx: u32, translation_x: f32) -> W3dPivot {
        W3dPivot {
            name: name.to_string(),
            parent_idx,
            translation: [translation_x, 0.0, 0.0],
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn hmodel_palette_source(model_name: &str, tree_name: &str, child_x: f32) -> W3DModel {
        let mut model = W3DModel::new(model_name.to_string());
        let hierarchy = W3dHierarchy {
            name: tree_name.to_string(),
            pivots: vec![pivot("ROOT", u32::MAX, 0.0), pivot("CHILD", 0, child_x)],
            pivot_fixups: Vec::new(),
        };
        model.hierarchies.push(hierarchy.clone());
        model.hierarchy = Some(hierarchy.clone());
        model.hmodels.push(W3dHmodel {
            version: 0x0004_0002,
            name: format!("{model_name}_HMODEL"),
            hierarchy_name: hierarchy.name,
            nodes: Vec::new(),
            source_snap_points: Vec::new(),
            has_invalid_records: false,
        });
        model
    }

    #[test]
    fn hmodel_skin_palette_resolves_only_explicit_source_and_fails_closed() {
        let hmodel_source = hmodel_palette_source("HMODEL_SOURCE", "HMODEL_TREE", 2.0);
        // This is the child mesh model and deliberately carries a different
        // hierarchy value. The palette source enum must prevent it from ever
        // being sampled for the HMODEL skin.
        let mesh_source = hmodel_palette_source("MESH_SOURCE", "WRONG_TREE", 99.0);
        let material = W3DMaterial::default();
        let mut item = RenderItem::new(
            ObjectId(41),
            "__strict_w3d_render_object_source__::skin_mesh".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &material,
            RenderPass::ForwardOpaque,
        );
        item.bone_palette_source = RenderItemBonePaletteSource::HmodelBindPose {
            source_model_cache_key: "__strict_w3d_render_object_source__::skin_hmodel".to_string(),
            hmodel_index: 0,
        };

        assert!(
            ForwardPass::sample_bone_palette_for_item(&mesh_source, &item).is_none(),
            "the single-model helper must not substitute the child mesh or parent model"
        );

        let models = HashMap::from([
            (
                "__strict_w3d_render_object_source__::skin_hmodel".to_string(),
                hmodel_source,
            ),
            (
                "__strict_w3d_render_object_source__::skin_mesh".to_string(),
                mesh_source,
            ),
        ]);
        let palette = ForwardPass::sample_bone_palette_for_item_with_model_resolver(&item, |key| {
            models.get(key)
        })
        .expect("the exact HMODEL source owns a bind-pose palette");
        assert_eq!(palette[0], Mat4::IDENTITY);
        assert_eq!(palette[1].w_axis.x, 2.0);
        assert_ne!(palette[1].w_axis.x, 99.0);

        item.bone_palette_source = RenderItemBonePaletteSource::HmodelBindPose {
            source_model_cache_key: "__strict_w3d_render_object_source__::missing".to_string(),
            hmodel_index: 0,
        };
        assert!(
            ForwardPass::sample_bone_palette_for_item_with_model_resolver(&item, |key| {
                models.get(key)
            })
            .is_none(),
            "a missing explicit palette source must not fall through to the mesh model"
        );

        item.bone_palette_source = RenderItemBonePaletteSource::HmodelBindPose {
            source_model_cache_key: "__strict_w3d_render_object_source__::skin_hmodel".to_string(),
            hmodel_index: 1,
        };
        assert!(
            ForwardPass::sample_bone_palette_for_item_with_model_resolver(&item, |key| {
                models.get(key)
            })
            .is_none(),
            "an out-of-range HMODEL definition must fail closed"
        );
    }

    #[test]
    fn ghost_light_environment_scales_frozen_ambient_and_directional_light() {
        let lighting = CachedLighting {
            sun_direction: Some([0.0, -1.0, 0.0]),
            sun_color: Some([0.8, 0.6, 0.4]),
            ambient_color: Some([0.4, 0.3, 0.2]),
            fog_color: None,
            fog_range: None,
            fogged_light_fraction: Some(0.25),
        };

        let ordinary = ForwardPass::build_light_environment(Some(&lighting))
            .expect("frozen lighting metadata builds an ordinary environment");
        let fogged = ForwardPass::build_always_fogged_light_environment(Some(&lighting))
            .expect("the same frozen metadata builds the dedicated ghost environment");

        assert_eq!(fogged.ambient, ordinary.ambient * 0.25);
        let ordinary_color = ordinary.lights[0]
            .lock()
            .expect("ordinary light lock")
            .color;
        let fogged_color = fogged.lights[0].lock().expect("ghost light lock").color;
        assert_eq!(fogged_color, ordinary_color * 0.25);
    }
}
