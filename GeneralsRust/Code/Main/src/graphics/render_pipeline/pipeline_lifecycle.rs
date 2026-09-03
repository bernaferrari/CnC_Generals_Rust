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
    /// Initialize render pipeline - equivalent to C++ RenderPipeline::Initialize()
    pub fn initialize(graphics_system: &GraphicsSystem) -> Result<Self> {
        info!("Initializing RenderPipeline (C++ SAGE equivalent)");

        // Initialize forward pass
        let forward_pass = ForwardPass::initialize(graphics_system)?;
        // C++ WW3DAssetManager::Get_Texture parity: let the ww3d mesh lane
        // hydrate W3D pass textures from the archive-backed TextureManager
        // (unit/building skins otherwise bind the white fallback).
        install_archive_pass_texture_provider(&forward_pass.renderer);
        let (ambient_light, sun_color, sun_direction) = graphics_system.current_lighting();
        let initial_lighting = CachedLighting {
            sun_direction: Some(sun_direction),
            sun_color: Some(sun_color),
            ambient_color: Some(ambient_light),
            fog_color: None,
            fog_range: None,
            fogged_light_fraction: None,
        };

        info!("RenderPipeline initialized successfully");

        Ok(Self {
            forward_pass,
            minimap_renderer: None, // Will be initialized when needed
            minimap_base_needs_refresh: false,
            heightmap_path_hint: None,
            pending_heightmap_hint_load: false,
            skybox_textures_hint: None,
            skybox_enabled: true,
            heightmap_world_size: None,
            cached_lighting: Some(initial_lighting.clone()),
            cached_terrain_lighting: Some(initial_lighting),
            last_startup_model_prewarm_signature: None,
            hlod_aggregate_prewarm_attempts: HashSet::new(),
            render_items: Vec::new(),
            frame_number: 0,
            current_pass: None,
            current_player_id: 0, // Default to player 0
            missing_ini_objects: HashSet::new(),
            debug_last_alive_objects: 0,
            debug_last_live_unit_identity_reads: 0,
            debug_last_presentation_live_fallback_reads: 0,
            debug_last_fow_filtered: 0,
            debug_last_frustum_culled: 0,
            debug_last_model_missing: 0,
            debug_last_deferred_model_loads: 0,
            debug_last_deferred_model_load_budget: 0,
            debug_last_model_budget_skips: 0,
            debug_last_zero_mesh_models: 0,
            debug_last_missing_model_samples: Vec::new(),
            debug_warned_bad_mesh_transforms: HashSet::new(),
            model_cull_bounds_cache: HashMap::new(),
            drawable_visual_states: HashMap::new(),
            pending_client_drawable_restore: None,
            pending_client_drawable_imports: HashMap::new(),
            #[cfg(feature = "game_client")]
            frozen_ghost_scene: None,
            last_frame_time: 0.0,
            presentation_frame: None,
            presentation_direct_shroud_states: HashMap::new(),
            presentation_direct_shroud_host_epoch: None,
            debug_last_laser_segments_packed: 0,
            debug_last_laser_pack_ok: false,
            debug_last_laser_gpu_write_ok: false,
            debug_last_projectile_segments_packed: 0,
            debug_last_projectile_pack_ok: false,
            debug_last_move_lines_packed: 0,
            debug_last_attack_lines_packed: 0,
            debug_last_floating_texts_packed: 0,
            debug_last_floating_text_pack_ok: false,
            debug_last_world_anims_packed: 0,
            debug_last_world_anim_pack_ok: false,
            debug_last_particle_systems_packed: 0,
            debug_last_particle_pack_ok: false,
            tactical_view_height_frac: 1.0,
            tactical_viewport_width: 1.0,
            tactical_viewport_height: 1.0,
        })
    }

    /// Provide full presentation snapshot for the next collect_render_items pass.
    pub fn set_presentation_frame(
        &mut self,
        frame: Option<crate::presentation_frame::PresentationFrame>,
    ) {
        // Direct-host shroud state is meaningful only for this exact frozen
        // topology. Main installs its replacement sidecar immediately after
        // this call; every other handoff intentionally stays empty rather
        // than reusing an object ID from an earlier frame.
        self.presentation_direct_shroud_states.clear();
        self.presentation_direct_shroud_host_epoch = None;
        // A queued v4 Drawable payload is meaningful only against the next
        // full frozen presentation topology.  Do the source-only identity
        // pass here, before normal collection.  This deliberately performs
        // no asset/archive I/O; collection consumes each resulting candidate
        // once alongside its ordinary model resolution.
        self.pending_client_drawable_imports.clear();
        if let Some(frame) = frame.as_ref() {
            self.prepare_pending_client_drawable_restore_for_frame(frame);
        }
        self.presentation_frame = frame;
    }

    /// C++ `W3DView::setHeight` — 3D viewport is the top `frac` of the window.
    pub fn set_tactical_3d_viewport(&mut self, width: f32, height: f32, frac: f32) {
        let frac = if frac.is_finite() {
            frac.clamp(0.05, 1.0)
        } else {
            1.0
        };
        self.tactical_view_height_frac = frac;
        self.tactical_viewport_width = width.max(1.0);
        self.tactical_viewport_height = height.max(1.0);
        self.forward_pass.tactical_view_height_frac = frac;
    }

    /// Install the immutable direct-host shroud sidecar for the frame just
    /// supplied to [`Self::set_presentation_frame`].
    ///
    /// Main obtains each entry through GameClient's guarded current binding
    /// query before calling this method. This renderer-owned method performs
    /// no GameClient/GameLogic read and merely replaces the current-frame map.
    pub fn set_presentation_direct_shroud_states<I>(&mut self, states: I)
    where
        I: IntoIterator<Item = FrozenDirectDrawableShroudState>,
    {
        self.presentation_direct_shroud_states.clear();
        self.presentation_direct_shroud_host_epoch = None;
        for state in states {
            if state.host_epoch == 0
                || state.object_id.0 == 0
                || state.drawable_id == 0
                || state.binding_generation == 0
            {
                continue;
            }
            match self.presentation_direct_shroud_host_epoch {
                None => self.presentation_direct_shroud_host_epoch = Some(state.host_epoch),
                Some(epoch) if epoch == state.host_epoch => {}
                // A renderer handoff must describe exactly one current host
                // world.  Mixed epochs are a malformed/stale batch, not a
                // reason to merge object IDs across world replacements.
                Some(_) => continue,
            }
            self.presentation_direct_shroud_states
                .insert(state.object_id, state);
        }
    }

    /// Discard renderer-local state tied to the current logical world.
    ///
    /// This is deliberately *not* part of [`Self::set_presentation_frame`]: a
    /// presentation frame can be absent during an ordinary handoff, while a
    /// successful map install, reset, or complete `GameLogic` replacement
    /// invalidates every raw object-id timeline.  Asset, terrain, minimap,
    /// lighting, and frame-counter caches have their own lifetimes and remain
    /// intact here.
    pub fn invalidate_world_visual_state(&mut self) {
        clear_visual_world_state_components(
            &mut self.drawable_visual_states,
            &mut self.render_items,
            &mut self.current_pass,
            &mut self.last_frame_time,
        );
        self.presentation_frame = None;
        self.presentation_direct_shroud_states.clear();
        self.presentation_direct_shroud_host_epoch = None;
        self.pending_client_drawable_restore = None;
        self.pending_client_drawable_imports.clear();
        #[cfg(feature = "game_client")]
        {
            self.frozen_ghost_scene = None;
        }
        self.hlod_aggregate_prewarm_attempts.clear();
    }

    #[inline]
    pub fn presentation_frame(&self) -> Option<&crate::presentation_frame::PresentationFrame> {
        self.presentation_frame.as_ref()
    }

    #[inline]
    pub fn presentation_frame_mut(
        &mut self,
    ) -> Option<&mut crate::presentation_frame::PresentationFrame> {
        self.presentation_frame.as_mut()
    }

    #[cfg(feature = "game_client")]
    pub fn frozen_ghost_scene(&self) -> Option<&game_client::render_bridge::FrozenGhostSceneFrame> {
        self.frozen_ghost_scene.as_ref()
    }

    /// Live GameLogic identity reads during last unit mesh collect (0 when presentation owns pass).
    pub fn last_live_unit_identity_reads(&self) -> usize {
        self.debug_last_live_unit_identity_reads
    }

    /// Live GameLogic dual-reads observed while presentation_frame was installed.
    /// Honesty residual: must remain 0 on the presentation-owned path.
    pub fn last_presentation_live_fallback_reads(&self) -> usize {
        self.debug_last_presentation_live_fallback_reads
    }

    /// Residual: presentation dual-read honesty — must stay 0 when frame installed.
    pub fn presentation_live_fallback_honesty_ok(&self) -> bool {
        self.debug_last_presentation_live_fallback_reads == 0
    }

    /// Pure unit-identity + FOW collection for the main mesh pass (no GameLogic borrow).
    ///
    /// Production `collect_render_items` uses this when a presentation frame is set.
    /// W3D mesh asset load remains outside this helper.
    pub fn collect_unit_render_inputs_from_presentation(
        frame: &crate::presentation_frame::PresentationFrame,
    ) -> Vec<crate::presentation_frame::UnitRenderInput> {
        // Wave 502: stealth filter/alpha applied inside unit_render_inputs (presentation-only).
        // Wave 503: disguise mesh swap + construction bits via stamp helper.
        // Wave 504: contained units filtered; garrisoned bits in stamp helper.
        // Wave 505: parachuting/jetexhaust/using-weapon stamp via unit_render_inputs.
        // Wave 506: weaponset veterancy stamp via unit_render_inputs.
        // Wave 507: over-water + transport RIDER stamp via unit_render_inputs.
        // Wave 508: body-damage / disguise / stun stamp via unit_render_inputs.
        // Wave 509: topple/freefall/night/snow stamp via unit_render_inputs.
        // Wave 510: captured/loaded/overcharge stamp via unit_render_inputs.
        // Wave 511: burned/aflame/cheer/carry stamp via unit_render_inputs.
        // Wave 512: continuous-fire/prone/preattack/turret stamp via unit_render_inputs.
        // Wave 513: jammed/dying/reload/packing stamp via unit_render_inputs.
        // Wave 515: surrender/raising-flag stamp via unit_render_inputs.
        frame.unit_render_inputs()
    }

    /// Backward-compatible: store IDs-only by building a minimal frame is not needed;
    /// prefer set_presentation_frame. Kept as thin alias for call sites.
    pub fn set_presentation_object_ids(&mut self, ids: Option<Vec<ObjectID>>) {
        if ids.is_none() {
            self.presentation_frame = None;
        }
        // IDs-only path no longer used; clear frame when None.
    }
}
