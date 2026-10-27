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

struct ShadowCallbackDevice {
    device: std::sync::Arc<wgpu::Device>,
}

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32-only. The single field is `Arc<wgpu::Device>`, an `Rc`-backed
// handle on the web backend that makes the struct `!Send`/`!Sync` by default.
// wasm32-unknown-unknown has no threads, so the device can never move to or
// be observed from another thread; the impl only satisfies the bounds of the
// shadow-pass callback slot that stores it. Native keeps the auto traits.
unsafe impl Send for ShadowCallbackDevice {}
#[cfg(target_arch = "wasm32")]
// SAFETY: same wasm32-only scope: the target is single-threaded, so
// `&ShadowCallbackDevice` is never accessed from two threads and the device's
// `!Sync` default is unreachable.
unsafe impl Sync for ShadowCallbackDevice {}
impl RenderPipeline {
    pub fn execute(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        #[cfg(feature = "game_client")]
        render_bridge: &mut game_client::render_bridge::RenderBridge,
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
        #[cfg(not(feature = "game_client"))] mut direct_scene_decision_resolver: Option<
            &mut dyn FnMut(
                &[FrozenDirectDrawableSceneCandidate],
            ) -> Vec<FrozenDirectDrawableSceneDecision>,
        >,
        #[cfg(feature = "game_client")] native_receiver: &mut dyn NativeFrameRenderReceiver,
        #[cfg(feature = "game_client")] native_radius_items: Vec<
            game_client::effects::decals::DecalRenderItem,
        >,
        #[cfg(feature = "game_client")] prepared_particles: Option<
            game_client::effects::particle_renderer::ParticleRenderFrame,
        >,
        #[cfg(feature = "game_client")] snow: &game_client::snow::SnowRenderState,
        #[cfg(feature = "game_client")] filter_scroll: glam::Vec2,
        #[cfg(feature = "game_client")] filter_logic_frame: u32,
        ui_overlay: Option<&mut crate::graphics::ui_render_pass::NativeUiOverlay<'_>>,
        #[cfg(not(feature = "game_client"))] complete_physics_visual: Option<
            &mut dyn FnMut(
                &mut crate::presentation_frame::UnitRenderInput,
                &crate::presentation_frame::PresentationFrame,
            ) -> std::result::Result<Option<Mat4>, String>,
        >,
    ) -> Result<()> {
        #[cfg(feature = "game_client")]
        let native_terrain_viewport = self.tactical_viewport.pixel_rect();
        #[cfg(feature = "game_client")]
        let native_terrain_clear_color = self.terrain_clear_color();

        let execute_started = std::time::Instant::now();
        #[cfg(feature = "game_client")]
        if let Some(lighting) = self
            .presentation_frame
            .as_ref()
            .and_then(|frame| Self::static_terrain_lighting(&frame.world_env))
        {
            if let Some(terrain) = self.native_terrain.as_mut() {
                terrain.admit_static_lighting(lighting);
            }
        }
        #[cfg(feature = "game_client")]
        if let Some(terrain) = self.native_terrain.as_mut() {
            let lights = self
                .presentation_frame
                .as_ref()
                .map_or(&[][..], |frame| frame.dynamic_lights.as_slice());
            terrain.admit_dynamic_lights(lights);
        }
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
                    self.presentation_frame
                        .as_ref()
                        .and_then(|frame| frame.world_env.runtime_heightmap.as_ref())
                        .map_or(0., |heightmap| heightmap.min_height),
                    native_radius_items,
                    prepared_particles,
                );
            }
            let _ = (laser, proj, moves, attacks, floats, anims, particles);
        }

        let delta_time = time - self.last_frame_time;
        self.last_frame_time = time;

        // The frozen session is the source of both scene and terrain light.
        // A changed time of day must replace the previous frame's values.
        self.sync_receiving_primary_lighting(graphics_system);

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
                #[cfg(feature = "game_client")]
                super::native_frame_receiver::NativeCollectSource::Native(&mut *native_receiver),
                #[cfg(not(feature = "game_client"))]
                complete_physics_visual,
            )?;
            collect_elapsed = collect_started.elapsed();
            #[cfg(feature = "game_client")]
            let direct_scene_decisions =
                native_receiver.direct_scene_decisions(&direct_scene_candidates);
            #[cfg(not(feature = "game_client"))]
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
                    render_bridge,
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
            #[cfg(feature = "game_client")]
            let _ = native_receiver.direct_scene_decisions(&direct_scene_candidates);
            #[cfg(not(feature = "game_client"))]
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
            let motion = self
                .presentation_frame
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("native tree draw requires receiving frame"))?
                .world_env
                .terrain_motion
                .native()
                .map_err(anyhow::Error::msg)?;
            let tree_receipt = {
                let (poses, terrain) = self.borrow_native_installed_poses_and_terrain();
                match terrain {
                    Some(terrain) => {
                        terrain.set_skybox_view(view_matrix);
                        native_receiver
                            .complete_tree_cpu_draw(terrain, poses, graphics_system, motion)
                            .map_err(anyhow::Error::msg)?
                    }
                    None => None,
                }
            };
            self.update_and_enqueue_terrain_pass_after_tree_cpu(
                view_matrix,
                projection_matrix,
                snow,
                tree_receipt.as_ref(),
            )?;
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
            let device = ShadowCallbackDevice {
                device: graphics_system.device_arc(),
            };
            let shadow_viewport_px = self.tactical_viewport_pixel_size();
            self.enqueue_post_frame_callback(move |gpu_frame| {
                let Some(depth_view) = gpu_frame.depth_view_arc() else {
                    return Ok(());
                };
                let color_view = gpu_frame.color_view_arc();
                game_client::display::shadow_pass::record_shadow_and_occlusion_passes(
                    device.device.as_ref(),
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

            // C++ W3DScene order: water (static sort level 2) renders after the
            // shadow passes and before translucent objects / post filters
            // (W3DScene.cpp:833-836). Enqueued after the shadow callback above,
            // so post-frame execution order is shadows -> water -> filter.
            self.execute_water_pass(graphics_system)?;
        }
        #[cfg(feature = "game_client")]
        let camera_fade = if shell_scene {
            crate::presentation_frame::PresentationCameraFade::default()
        } else {
            self.presentation_frame
                .as_ref()
                .map(|frame| frame.camera_fade)
                .unwrap_or_default()
        };
        #[cfg(feature = "game_client")]
        let skip_filter_scene = native_receiver.view_filter().filter()
            == game_client::display::view::FilterType::MotionBlur
            && native_receiver.view_filter().skip_render()
            && ww3d_engine::surface_size()
                .ok()
                .is_some_and(|(width, height)| {
                    self.view_filter_resources.has_scene_capture_for(
                        width,
                        height,
                        graphics_system.color_format(),
                    )
                });
        #[cfg(feature = "game_client")]
        if skip_filter_scene {
            // Reusing an image is not a new accepted draw of its ghost objects.
            self.committed_host_ghost_draws.clear();
        }
        #[cfg(feature = "game_client")]
        let locked_binding = if !skip_filter_scene
            && native_receiver.view_filter().filter()
                != game_client::display::view::FilterType::Null
            && native_receiver.view_filter().mode() != game_client::display::view::FilterMode::Null
        {
            filter_scene::camera_lock_binding(
                self.presentation_frame
                    .as_ref()
                    .and_then(|f| f.camera_follow_object_id),
                &self.presentation_direct_shroud_states,
                self.presentation_direct_shroud_host_epoch,
                &direct_scene_candidates,
            )
        } else {
            None
        };
        #[cfg(feature = "game_client")]
        let filter_resources = &mut self.view_filter_resources;
        #[cfg(feature = "game_client")]
        let mut native_poses = self
            .native_draw_world
            .borrow_installed_poses(&mut self.failed_model_resolutions, self.animation_sync_ms);
        #[cfg(feature = "game_client")]
        let mut finish_filters = |gpu_frame: &mut ww3d_engine::RenderFrame,
                                  graphics: &mut GraphicsSystem| {
            let mut look_at = None;
            let dest = gpu_frame.color_view_arc();
            let format = gpu_frame.color_format();
            let device = gpu_frame.device_arc();
            let queue = gpu_frame.queue_arc();
            let (color_texture, encoder) = gpu_frame.color_texture_and_encoder();
            let width = color_texture.width();
            let height = color_texture.height();
            let scroll = game_client::display::view::Vector2::new(filter_scroll.x, filter_scroll.y);
            {
                let view_filter = native_receiver.view_filter_mut();
                let composite = view_filter.composite(scroll);
                if filter_resources.prepare(
                    device.as_ref(),
                    queue.as_ref(),
                    format,
                    width,
                    height,
                    &composite,
                ) {
                    view_filter.tick_fade();
                    let composite = view_filter.composite(scroll);
                    let post = view_filter.post_render(filter_logic_frame, scroll);
                    filter_resources.composite_retained_view_filter(
                        device.as_ref(),
                        queue.as_ref(),
                        encoder,
                        dest.as_ref(),
                        color_texture,
                        format,
                        width,
                        height,
                        &composite,
                        &post,
                        !skip_filter_scene,
                    );
                    look_at = post
                        .look_at
                        .map(|position| Vec3::new(position.x, position.z, position.y));
                }
            }
            // C++ LookAt executes inside postRender, before the locked Drawable
            // uses the new matrix. This is not another camera/view update.
            native_receiver
                .complete_filter_camera(look_at, &mut native_poses, graphics)
                .map(Some)
                .map_err(ww3d_renderer_3d::Error::InvalidOperation)
        };
        #[cfg(feature = "game_client")]
        let fade_resources = &mut self.camera_fade_resources;
        #[cfg(feature = "game_client")]
        let mut finish_overlay = |frame: &mut ww3d_engine::RenderFrame| {
            let dest = frame.color_view_arc();
            let device = frame.device_arc();
            let queue = frame.queue_arc();
            let format = frame.color_format();
            let encoder = frame.encoder();
            game_client::display::status_circle::record_camera_fade(
                fade_resources,
                device.as_ref(),
                queue.as_ref(),
                encoder,
                dest.as_ref(),
                format,
                camera_fade.fade,
                camera_fade.intensity,
                camera_fade.diffuse,
            );
            Ok(())
        };
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
            #[cfg(feature = "game_client")]
            skip_filter_scene,
            #[cfg(not(feature = "game_client"))]
            false,
            #[cfg(feature = "game_client")]
            Some(&mut finish_filters),
            #[cfg(not(feature = "game_client"))]
            None,
            #[cfg(feature = "game_client")]
            locked_binding,
            #[cfg(not(feature = "game_client"))]
            None,
            #[cfg(feature = "game_client")]
            Some(&mut finish_overlay),
            #[cfg(not(feature = "game_client"))]
            None,
            ui_overlay,
            #[cfg(feature = "game_client")]
            self.native_terrain.as_ref(),
            #[cfg(feature = "game_client")]
            native_terrain_viewport,
            #[cfg(feature = "game_client")]
            native_terrain_clear_color,
        )?;

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

    fn sync_receiving_primary_lighting(&mut self, graphics_system: &mut GraphicsSystem) {
        let Some(frame) = self.presentation_frame.as_ref() else {
            return;
        };
        let object = frame.world_env.primary_object_lighting;
        let terrain = frame.world_env.primary_terrain_lighting;
        if let Some(light) = object {
            let state = self
                .cached_lighting
                .get_or_insert_with(CachedLighting::default);
            let direction = light.object_light_active.then(|| light.render_light_pos());
            let diffuse = light.object_light_active.then_some(light.diffuse);
            let changed = state.ambient_color != Some(light.ambient)
                || direction.is_some_and(|value| state.sun_direction != Some(value))
                || diffuse.is_some_and(|value| state.sun_color != Some(value));
            state.ambient_color = Some(light.ambient);
            if let Some(direction) = direction {
                state.sun_direction = Some(direction);
            }
            if let Some(diffuse) = diffuse {
                state.sun_color = Some(diffuse);
            }
            if changed {
                graphics_system.set_lighting(Some(light.ambient), diffuse, direction, None);
            }
        }
        if let Some(light) = terrain {
            let state = self
                .cached_terrain_lighting
                .get_or_insert_with(CachedLighting::default);
            let direction = light.render_light_pos();
            let changed = state.ambient_color != Some(light.ambient)
                || state.sun_direction != Some(direction)
                || state.sun_color != Some(light.diffuse);
            state.ambient_color = Some(light.ambient);
            state.sun_direction = Some(direction);
            state.sun_color = Some(light.diffuse);
            if changed {
                let state = state.clone();
                self.apply_cached_lighting_to_terrain(&state);
            }
        }
    }

    pub(super) fn sync_lighting_from_map_metadata(&mut self) {
        // Presentation-only: frozen world_env (no live map-settings re-read).
        let Some(pres) = self.presentation_frame.as_ref() else {
            return;
        };
        #[cfg(feature = "game_client")]
        {
            let lighting = self
                .cached_lighting
                .get_or_insert_with(CachedLighting::default);
            lighting.dynamic_lights.clone_from(&pres.dynamic_lights);
        }
        let env = &pres.world_env;
        if !env.has_map_metadata
            && env.sun_direction.is_none()
            && env.sun_color.is_none()
            && env.ambient_color.is_none()
        {
            return;
        }
        let derived = CachedLighting {
            #[cfg(feature = "game_client")]
            dynamic_lights: Vec::new(),
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

#[cfg(feature = "game_client")]
impl RenderPipeline {
    pub(crate) fn capture_native_particles(
        &self,
        manager: &mut game_client::effects::particle_manager::ParticleSystemManager,
        view: Mat4,
        projection: Mat4,
        position: Vec3,
        time: f32,
    ) -> Option<game_client::effects::particle_renderer::ParticleRenderFrame> {
        let min_height = self
            .presentation_frame
            .as_ref()
            .and_then(|frame| frame.world_env.runtime_heightmap.as_ref())
            .map_or(0., |heightmap| heightmap.min_height);
        ForwardPass::prepare_client_particles(manager, view, projection, position, time, min_height)
    }
}
