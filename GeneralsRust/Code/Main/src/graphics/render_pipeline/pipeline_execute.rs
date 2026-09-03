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
    pub fn execute(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        // Immutable presentation boundary: world collect reads only
        // `self.presentation_frame` (engine always seeds a frame before execute).
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        time: f32,
        allow_sync_model_loads: bool,
        deferred_startup_model_load_budget: usize,
        skip_world_scene: bool,
        // Called synchronously after frozen collection and before sort/forward.
        // Main owns this callback and may resolve candidates against
        // GameClient; RenderPipeline itself remains free of GameClient/live
        // logic access. It returns Main-owned, full-keyed decisions that are
        // revalidated against this exact frozen sidecar before item mutation.
        mut direct_scene_decision_resolver: Option<
            &mut dyn FnMut(
                &[FrozenDirectDrawableSceneCandidate],
            ) -> Vec<FrozenDirectDrawableSceneDecision>,
        >,
    ) -> Result<()> {
        let execute_started = std::time::Instant::now();
        trace!("RenderPipeline::execute frame {}", self.frame_number + 1);
        if (self.frame_number + 1).is_multiple_of(300) {
            debug!(
                "RenderPipeline frame {} - {} objects queued",
                self.frame_number + 1,
                self.render_items.len()
            );
        }

        self.frame_number += 1;
        if self.frame_number <= 5 {
            info!(
                "RenderPipeline::execute frame {} start (skip_world_scene={})",
                self.frame_number, skip_world_scene
            );
        }
        graphics_system.begin_frame();

        // Presentation-only FX: pack lasers/projectiles/order lines/particles from the
        // frozen frame (no live GameLogic dual-read). Laser SegLine then uploads via
        // `Queue::write_buffer` when the ForwardPass buffer exists (same path as particles).
        {
            let mut laser = self.pack_presentation_laser_segments();
            self.debug_last_laser_segments_packed = laser.honesty.segments_packed;
            self.debug_last_laser_pack_ok = laser.honesty.cpu_pack_ok;
            self.debug_last_laser_gpu_write_ok =
                self.forward_pass.upload_laser_segments(&mut laser);
            if self.debug_last_laser_gpu_write_ok {
                let _ = self
                    .forward_pass
                    .enqueue_laser_additive_draw(view_matrix, projection_matrix);
            }
            let proj = self.pack_presentation_projectiles();
            self.debug_last_projectile_segments_packed = proj.honesty.projectiles_packed;
            self.debug_last_projectile_pack_ok = proj.honesty.cpu_pack_ok;
            let moves = self.pack_presentation_move_lines();
            self.debug_last_move_lines_packed = moves.honesty.lines_packed;
            let attacks = self.pack_presentation_attack_lines();
            self.debug_last_attack_lines_packed = attacks.honesty.lines_packed;
            let floats = self.pack_presentation_floating_texts();
            self.debug_last_floating_texts_packed = floats.honesty.texts_packed;
            self.debug_last_floating_text_pack_ok = floats.honesty.cpu_pack_ok;
            let anims = self.pack_presentation_world_anims();
            self.debug_last_world_anims_packed = anims.honesty.anims_packed;
            self.debug_last_world_anim_pack_ok = anims.honesty.cpu_pack_ok;
            let particles = self.pack_presentation_particle_systems();
            self.debug_last_particle_systems_packed = particles.honesty.systems_packed;
            self.debug_last_particle_pack_ok = particles.honesty.cpu_pack_ok;
            #[cfg(feature = "game_client")]
            {
                // The packed frame remains an immutable presentation diagnostic,
                // while the live WGPU draw consumes the GameClient systems that
                // `host_tick_game_client_presentation_shell` already advanced.
                // This queues onto Main's sole frame; it never calls Display::draw.
                let _ = self.forward_pass.enqueue_client_effect_draw(
                    view_matrix,
                    projection_matrix,
                    camera_position,
                    time,
                );
            }
            let _ = (laser, proj, moves, attacks, floats, anims, particles);
        }

        let delta_time = time - self.last_frame_time;
        self.last_frame_time = time;

        // Update global uniforms
        graphics_system.update_global_uniforms(
            view_matrix,
            projection_matrix,
            camera_position,
            time,
        );
        // Removed excessive logging

        // Clear render items from previous frame
        self.render_items.clear();

        let render_world_scene = !skip_world_scene;

        let mut collect_elapsed = std::time::Duration::ZERO;
        let mut sort_elapsed = std::time::Duration::ZERO;
        let mut terrain_elapsed = std::time::Duration::ZERO;
        // Bounded to the current execute call. The collector emits at most
        // one record per complete direct Drawable binding, regardless of the
        // number of W3D Draw modules or meshes that produced render items.
        let mut direct_scene_candidates = Vec::new();
        if render_world_scene {
            self.sync_lighting_from_map_metadata();
            if allow_sync_model_loads {
                if self.frame_number <= 5 {
                    info!(
                        "RenderPipeline::execute frame {} prewarm_start",
                        self.frame_number
                    );
                }
                self.prewarm_startup_models(graphics_system, allow_sync_model_loads);
                self.prewarm_frozen_draw_animation_bindings();
                self.prewarm_cached_hlod_aggregate_render_objects(graphics_system);
                if self.frame_number <= 5 {
                    info!(
                        "RenderPipeline::execute frame {} prewarm_done",
                        self.frame_number
                    );
                }
            }

            // Shell/menu startup needs to make visible progress without stalling first paint.
            let mut deferred_model_load_budget = if allow_sync_model_loads {
                usize::MAX
            } else {
                deferred_startup_model_load_budget
            };
            let initial_deferred_model_load_budget = if allow_sync_model_loads {
                0
            } else {
                deferred_model_load_budget
            };
            self.debug_last_model_budget_skips = 0;
            self.debug_last_zero_mesh_models = 0;
            self.debug_last_missing_model_samples.clear();
            self.debug_warned_bad_mesh_transforms.clear();

            // Collect render items from game objects - equivalent to C++ RenderPipeline::CollectRenderItems()
            let collect_started = std::time::Instant::now();
            if self.frame_number <= 5 {
                info!(
                    "RenderPipeline::execute frame {} collect_start (items={})",
                    self.frame_number,
                    self.render_items.len()
                );
            }
            self.collect_render_items(
                graphics_system,
                view_matrix,
                projection_matrix,
                camera_position,
                allow_sync_model_loads,
                &mut deferred_model_load_budget,
                delta_time,
                &mut direct_scene_candidates,
            )?;
            collect_elapsed = collect_started.elapsed();
            let direct_scene_decisions = direct_scene_decision_resolver
                .as_deref_mut()
                .map_or_else(Vec::new, |resolver| resolver(&direct_scene_candidates));
            // This stays after collection (and GameClient's callback) but
            // before bridge submissions, sort, and forward render. It never
            // reads FOW alpha: the callback's final C++ scene ordinal is the
            // sole authority for the future projected-shroud material pass.
            apply_frozen_direct_scene_decisions_to_render_items(
                &mut self.render_items,
                &self.presentation_direct_shroud_states,
                self.presentation_direct_shroud_host_epoch,
                &direct_scene_candidates,
                direct_scene_decisions,
            );
            if self.frame_number <= 5 {
                info!(
                    "RenderPipeline::execute frame {} collect_done ({} items, {:?})",
                    self.frame_number,
                    self.render_items.len(),
                    collect_elapsed
                );
            }
            self.debug_last_deferred_model_load_budget = initial_deferred_model_load_budget;
            self.debug_last_deferred_model_loads = if allow_sync_model_loads {
                0
            } else {
                initial_deferred_model_load_budget.saturating_sub(deferred_model_load_budget)
            };

            #[cfg(feature = "game_client")]
            {
                self.drain_render_bridge_submissions(
                    graphics_system,
                    camera_position,
                    &mut deferred_model_load_budget,
                );
                self.append_frozen_mesh_ghost_scene(
                    graphics_system,
                    camera_position,
                    allow_sync_model_loads,
                    &mut deferred_model_load_budget,
                );
            }

            // Sort render items for optimal rendering - equivalent to C++ RenderPipeline::SortRenderItems()
            let sort_started = std::time::Instant::now();
            self.sort_render_items();
            sort_elapsed = sort_started.elapsed();
            // Removed excessive logging

            static LOGGED_STARTUP_RENDER_ITEM_SUMMARY: AtomicBool = AtomicBool::new(false);
            if !self.render_items.is_empty()
                && !LOGGED_STARTUP_RENDER_ITEM_SUMMARY.swap(true, Ordering::Relaxed)
            {
                let sample_items: Vec<String> = self
                    .render_items
                    .iter()
                    .take(12)
                    .map(|item| format!("{}#{}", item.model_name, item.mesh_index))
                    .collect();
                info!(
                    "Startup render summary: render_items={} sample_models={:?}",
                    self.render_items.len(),
                    sample_items
                );
            }
        } else {
            if let Some(resolver) = direct_scene_decision_resolver.as_deref_mut() {
                // Preserve the callback contract for a deliberately skipped
                // world scene without carrying decisions into a later frame.
                let _ = resolver(&direct_scene_candidates);
            }
            self.debug_last_deferred_model_load_budget = 0;
            self.debug_last_deferred_model_loads = 0;
            self.debug_last_model_budget_skips = 0;
            self.debug_last_zero_mesh_models = 0;
            self.debug_last_missing_model_samples.clear();
            self.debug_last_alive_objects = 0;
            self.debug_last_fow_filtered = 0;
            self.debug_last_model_missing = 0;
        }

        let shell_scene = self
            .presentation_frame
            .as_ref()
            .map(|p| p.fow_shell_bypass)
            .unwrap_or(false);
        if render_world_scene && !shell_scene {
            // Presentation-owned bounds/heights when frame is set; live GameLogic
            // is only a boot fallback (execute already passes None with snapshot).
            if let Err(e) = self.refresh_minimap_terrain_base() {
                error!("Failed to refresh minimap terrain base: {}", e);
            }

            // Update minimap FOW texture before rendering UI
            if let Err(e) = self.update_minimap_fow_texture() {
                error!("Failed to update minimap FOW texture: {}", e);
            }
        }

        #[cfg(feature = "game_client")]
        if render_world_scene {
            let terrain_started = std::time::Instant::now();
            self.update_and_enqueue_terrain_pass(view_matrix, projection_matrix)?;
            terrain_elapsed = terrain_started.elapsed();
        }

        let forward_started = std::time::Instant::now();
        if self.frame_number <= 5 {
            info!(
                "RenderPipeline::execute frame {} forward_pass_start (items={})",
                self.frame_number,
                self.render_items.len()
            );
        }
        let projected_shroud = self
            .presentation_frame
            .as_ref()
            .and_then(|frame| frame.terrain_projected_shroud());
        self.forward_pass.render(
            graphics_system,
            &self.render_items,
            view_matrix,
            projection_matrix,
            camera_position,
            self.cached_lighting.as_ref(),
            projected_shroud,
        )?;
        #[cfg(feature = "game_client")]
        {
            // ZPROBE (documented diagnostic, GENERALS_ZPROBE=1, one-shot): dump the
            // mesh-lane clip z/w per render item next to the GPU depth the terrain
            // pre-scene pass wrote at the same pixels. The readback runs on its own
            // encoder inside the post-frame callback, so it observes the PREVIOUS
            // frame's final depth attachment (the attachment persists across frames;
            // the terrain pass clears it and draws first). Fully synchronous:
            // submit, poll, map, read, unmap — no cross-frame mapped window.
            // Fires on the first IN-MATCH world-scene frame that queued items
            // (execute also runs for Loading/Menu shell frames with zero items);
            // logs at warn because main.rs filters generals_main::graphics to Warn.
            static ZPROBE_DONE: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            static ZPROBE_ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
                std::env::var("GENERALS_ZPROBE").as_deref() == Ok("1")
            });
            if *ZPROBE_ENABLED
                && render_world_scene
                && !shell_scene
                && !self.render_items.is_empty()
                && !ZPROBE_DONE.swap(true, std::sync::atomic::Ordering::Relaxed)
            {
                let view_proj = *projection_matrix * *view_matrix;
                let mut report = String::from("ZPROBE item clip-z dump:");
                let mut samples: Vec<(i32, i32, f32)> = Vec::new();
                let mut seen_models: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                let mut sampled = 0usize;
                for (i, item) in self.render_items.iter().enumerate() {
                    if sampled >= 16 {
                        break;
                    }
                    // One sample per model family: the item list repeats a base
                    // building's meshes many times; distinct families cover both
                    // buried-origin buildings and on-ground units (dozer).
                    if !seen_models.insert(item.model_name.clone()) {
                        continue;
                    }
                    let p = item.world_position;
                    let clip = view_proj * p.extend(1.0);
                    if !(clip.w > 0.001) || !clip.z.is_finite() {
                        continue;
                    }
                    let z_over_w = clip.z / clip.w;
                    let ndc = clip.truncate() / clip.w;
                    if !(-1.1..=1.1).contains(&ndc.x) || !(-1.1..=1.1).contains(&ndc.y) {
                        continue;
                    }
                    let px = ((ndc.x * 0.5 + 0.5) * 640.0).clamp(0.0, 639.0) as i32;
                    let py = ((0.5 - ndc.y * 0.5) * 480.0).clamp(0.0, 479.0) as i32;
                    let ground = game_client::terrain::terrain_visual::get_terrain_visual()
                        .ok()
                        .and_then(|mut guard| {
                            guard
                                .as_mut()
                                .and_then(|visual| visual.get_height_at(p.x, p.z).ok())
                        })
                        .unwrap_or(f32::NAN);
                    let ground_clip = view_proj * Vec4::new(p.x, ground, p.z, 1.0);
                    let ground_z = if ground_clip.w > 0.001 {
                        ground_clip.z / ground_clip.w
                    } else {
                        f32::NAN
                    };
                    report.push_str(&format!(
                        "\n  item{i} model={} world=({:.1},{:.2},{:.1}) mesh_z/w={:.6} px=({},{}) ground_h={:.2} ground_z/w={:.6}",
                        item.model_name, p.x, p.y, p.z, z_over_w, px, py, ground, ground_z
                    ));
                    samples.push((px, py, z_over_w));
                    sampled += 1;
                }
                warn!("{}", report);

                self.enqueue_post_frame_callback(move |gpu_frame| {
                    let Some(depth_view) = gpu_frame.depth_view_arc() else {
                        return Ok(());
                    };
                    let texture = depth_view.texture();
                    let size = texture.size();
                    let (w, h) = (size.width, size.height);
                    let bytes_per_row = (w * 4).div_ceil(256) * 256;
                    let device = gpu_frame.device_arc();
                    let queue = gpu_frame.queue_arc();
                    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("ZPROBE depth readback"),
                        size: bytes_per_row as u64 * h as u64,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    });
                    let mut encoder = device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("ZPROBE depth copy"),
                        },
                    );
                    encoder.copy_texture_to_buffer(
                        wgpu::TexelCopyTextureInfo {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyBufferInfo {
                            buffer: &buffer,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(bytes_per_row),
                                rows_per_image: Some(h),
                            },
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                    queue.submit(Some(encoder.finish()));
                    let slice = buffer.slice(..);
                    let (tx, rx) = std::sync::mpsc::channel();
                    slice.map_async(wgpu::MapMode::Read, move |result| {
                        let _ = tx.send(result);
                    });
                    let _ = device
                        .poll(wgpu::PollType::Wait {
                            submission_index: None,
                            timeout: None,
                        });
                    if rx.recv_timeout(std::time::Duration::from_secs(5)).is_err() {
                        warn!("ZPROBE depth map timed out");
                        return Ok(());
                    }
                    let data = slice.get_mapped_range();
                    let read = |px: i32, py: i32| -> f32 {
                        let px = px.clamp(0, w as i32 - 1) as usize;
                        let py = py.clamp(0, h as i32 - 1) as usize;
                        let off = py * bytes_per_row as usize + px * 4;
                        f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
                    };
                    let mut line = String::from("ZPROBE GPU depth readback (previous frame):");
                    for (px, py, mesh_z) in &samples {
                        // 3x3 neighborhood: the exact pixel may be a nearer
                        // occluder (or the item's own buried origin); neighbors
                        // expose whether ANY adjacent depth accepts the mesh z.
                        let mut min_d = f32::INFINITY;
                        let mut max_d = f32::NEG_INFINITY;
                        for dy in -1..=1 {
                            for dx in -1..=1 {
                                let d = read(px + dx, py + dy);
                                min_d = min_d.min(d);
                                max_d = max_d.max(d);
                            }
                        }
                        line.push_str(&format!(
                            "\n  px=({},{}) gpu_depth={:.6} min3x3={:.6} max3x3={:.6} mesh_z/w={:.6}",
                            px,
                            py,
                            read(*px, *py),
                            min_d,
                            max_d,
                            mesh_z
                        ));
                    }
                    line.push_str(&format!(
                        "\n  center px=({},{}) gpu_depth={:.6}",
                        w as i32 / 2,
                        h as i32 / 2,
                        read(w as i32 / 2, h as i32 / 2)
                    ));
                    // Clear-value check: nothing but the terrain pre-scene pass
                    // (Clear 1.0 + Less) is expected to write this attachment, so
                    // untouched pixels must read 1.0. A 0.0 corner means the
                    // observed attachment was never cleared by that lane.
                    for (cx, cy, tag) in [
                        (0, 0, "tl"),
                        (w as i32 - 1, 0, "tr"),
                        (0, h as i32 - 1, "bl"),
                        (w as i32 - 1, h as i32 - 1, "br"),
                    ] {
                        line.push_str(&format!(
                            "\n  corner {} ({},{}) gpu_depth={:.6}",
                            tag,
                            cx,
                            cy,
                            read(cx, cy)
                        ));
                    }
                    // Center-column profile: a monotone far-to-near ramp is a
                    // sane terrain surface; plateaus/off-formula values indict a
                    // specific writer or a lane z mismatch.
                    line.push_str("\n  column px=320 profile:");
                    for py in (8..h as i32).step_by(32) {
                        line.push_str(&format!(" {:.6}", read(320, py)));
                    }
                    drop(data);
                    buffer.unmap();
                    warn!("{}", line);
                    Ok(())
                });
            }
        }
        #[cfg(feature = "game_client")]
        {
            // C++ DoShadows(true) after opaque flush.
            let depth_format = graphics_system
                .depth_format()
                .unwrap_or(wgpu::TextureFormat::Depth32Float);
            let color_format = graphics_system.color_format();
            let _ = game_client::display::shadow_pass::present_volumetric_shadows(depth_format);
            let view_proj = *projection_matrix * *view_matrix;
            let camera = [camera_position.x, camera_position.y, camera_position.z];
            let light_pos = self
                .cached_lighting
                .as_ref()
                .and_then(|lighting| lighting.sun_direction)
                .unwrap_or([0.0, 0.0, -1.0]);
            let device = graphics_system.device_arc();
            let shadow_viewport_px = self.tactical_viewport_pixel_size();
            self.enqueue_post_frame_callback(move |gpu_frame| {
                let Some(depth_view) = gpu_frame.depth_view_arc() else {
                    return Ok(());
                };
                let color_view = gpu_frame.color_view_arc();
                game_client::display::shadow_pass::record_shadow_and_occlusion_passes(
                    device.as_ref(),
                    gpu_frame.encoder(),
                    color_view.as_ref(),
                    depth_view.as_ref(),
                    view_proj,
                    camera,
                    light_pos,
                    color_format,
                    depth_format,
                    shadow_viewport_px,
                );
                Ok(())
            });
            // C++ W3DView::draw filterPostRender + W3DStatusCircle fade overlay.
            // Leftover Display::draw is not the live 3D path.
            // `filter_composite.scroll_delta` is leftover View.scroll_amount,
            // stamped same-frame by live `camera_scroll_world_delta`.
            let filter_composite = {
                game_client::display::view::with_tactical_view(|view| {
                    view.tick_filter_fade();
                });
                game_client::display::view::with_tactical_view_ref(|view| view.filter_composite())
            };
            let camera_fade = self
                .presentation_frame
                .as_ref()
                .map(|frame| frame.camera_fade)
                .unwrap_or_default();
            self.enqueue_post_frame_callback(move |gpu_frame| {
                let dest = gpu_frame.color_view_arc();
                let format = gpu_frame.color_format();
                let device = gpu_frame.device_arc();
                let queue = gpu_frame.queue_arc();
                let (color_texture, encoder) = gpu_frame.color_texture_and_encoder();
                let width = color_texture.width();
                let height = color_texture.height();
                game_client::display::shader_filter::composite_live_view_filter(
                    device.as_ref(),
                    queue.as_ref(),
                    encoder,
                    dest.as_ref(),
                    color_texture,
                    format,
                    width,
                    height,
                    &filter_composite,
                );
                let fade = game_client::display::status_circle::take_queued_live_camera_fade()
                    .map(|overlay| (overlay.fade as u8, overlay.intensity, overlay.diffuse))
                    .unwrap_or((camera_fade.fade, camera_fade.intensity, camera_fade.diffuse));
                game_client::display::status_circle::record_camera_fade(
                    device.as_ref(),
                    queue.as_ref(),
                    encoder,
                    dest.as_ref(),
                    format,
                    fade.0,
                    fade.1,
                    fade.2,
                );
                Ok(())
            });
        }

        let forward_elapsed = forward_started.elapsed();
        if self.frame_number <= 5 {
            info!(
                "RenderPipeline::execute frame {} forward_pass_done ({:?})",
                self.frame_number, forward_elapsed
            );
        }

        graphics_system.end_frame();
        if render_world_scene && !shell_scene {
            self.maybe_load_heightmap_hint_after_first_present(graphics_system);
        }

        // Removed excessive logging
        let execute_elapsed = execute_started.elapsed();
        if execute_elapsed >= std::time::Duration::from_millis(200) {
            warn!(
                "RenderPipeline breakdown: total={:?} collect={:?} sort={:?} terrain={:?} forward={:?} render_world_scene={} render_items={} model_missing={} deferred_loads={}/{}",
                execute_elapsed,
                collect_elapsed,
                sort_elapsed,
                terrain_elapsed,
                forward_elapsed,
                render_world_scene,
                self.render_items.len(),
                self.debug_last_model_missing,
                self.debug_last_deferred_model_loads,
                self.debug_last_deferred_model_load_budget
            );
        }
        Ok(())
    }

    pub(super) fn sync_lighting_from_map_metadata(&mut self) {
        // Presentation-only: frozen world_env (no live map-settings re-read).
        let Some(pres) = self.presentation_frame.as_ref() else {
            return;
        };
        let env = &pres.world_env;
        if !env.has_map_metadata
            && env.sun_direction.is_none()
            && env.sun_color.is_none()
            && env.ambient_color.is_none()
        {
            return;
        }
        let derived = CachedLighting {
            sun_direction: env.sun_direction,
            sun_color: env.sun_color,
            ambient_color: env.ambient_color,
            fog_color: env.fog_color,
            fog_range: env.fog_range(),
            fogged_light_fraction: Some(env.fogged_light_fraction()),
        };

        match &mut self.cached_lighting {
            Some(existing) => {
                if existing.sun_direction.is_none() {
                    existing.sun_direction = derived.sun_direction;
                }
                if existing.sun_color.is_none() {
                    existing.sun_color = derived.sun_color;
                }
                if existing.ambient_color.is_none() {
                    existing.ambient_color = derived.ambient_color;
                }
                if existing.fog_color.is_none() {
                    existing.fog_color = derived.fog_color;
                }
                if existing.fog_range.is_none() {
                    existing.fog_range = derived.fog_range;
                }
                if existing.fogged_light_fraction.is_none() {
                    existing.fogged_light_fraction = derived.fogged_light_fraction;
                }
            }
            None => {
                self.cached_lighting = Some(derived);
            }
        }
    }
}
